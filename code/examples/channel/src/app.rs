use eyre::eyre;
use tracing::{error, info};

use malachite_app_channel::app::host::LocallyProposedValue;
use malachite_app_channel::app::types::core::{Round, Validity};
use malachite_app_channel::app::types::ProposedValue;
use malachite_app_channel::{AppMsg, Channels, ConsensusMsg, NetworkMsg};
use malachite_test::{Genesis, TestContext};

use crate::state::{decode_value, State};

pub async fn run(
    genesis: Genesis,
    state: &mut State,
    channels: &mut Channels<TestContext>,
) -> eyre::Result<()> {
    while let Some(msg) = channels.consensus.recv().await {
        match msg {
            AppMsg::ConsensusReady { reply } => {
                info!("Consensus is ready");

                if reply
                    .send(ConsensusMsg::StartHeight(
                        state.current_height,
                        genesis.validator_set.clone(),
                    ))
                    .is_err()
                {
                    error!("Failed to send ConsensusReady reply");
                }
            }

            AppMsg::StartedRound {
                height,
                round,
                proposer,
            } => {
                info!(%height, %round, %proposer, "Started round");

                state.current_height = height;
                state.current_round = round;
                state.current_proposer = Some(proposer);
            }

            AppMsg::GetValue {
                height,
                round,
                timeout: _,
                reply,
            } => {
                info!(%height, %round, "Get value");

                let proposal = state.propose_value(&height);

                let value = LocallyProposedValue::new(
                    proposal.height,
                    proposal.round,
                    proposal.value,
                    proposal.extension,
                );

                // Send it to consensus
                if reply.send(value.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                let stream_message = state.create_stream_message(value);

                // Broadcast it to others. Old messages need not be broadcast.
                channels
                    .network
                    .send(NetworkMsg::PublishProposalPart(stream_message))
                    .await?;
            }

            AppMsg::GetHistoryMinHeight { reply } => {
                if reply.send(state.get_earliest_height()).is_err() {
                    error!("Failed to send GetHistoryMinHeight reply");
                }
            }

            AppMsg::ReceivedProposalPart {
                from: _,
                part,
                reply,
            } => {
                if let Some(proposed_value) = state.add_proposal(part) {
                    if reply.send(proposed_value).is_err() {
                        error!("Failed to send ReceivedProposalPart reply");
                    }
                }
            }

            AppMsg::GetValidatorSet { height: _, reply } => {
                if reply.send(genesis.validator_set.clone()).is_err() {
                    error!("Failed to send GetValidatorSet reply");
                }
            }

            AppMsg::Decided { certificate, reply } => {
                state.commit(certificate);

                if reply
                    .send(ConsensusMsg::StartHeight(
                        state.current_height,
                        genesis.validator_set.clone(),
                    ))
                    .is_err()
                {
                    error!("Failed to send Decided reply");
                }
            }

            AppMsg::GetDecidedValue { height, reply } => {
                let decided_value = state.get_decided_value(&height).cloned();

                if reply.send(decided_value).is_err() {
                    error!("Failed to send GetDecidedValue reply");
                }
            }

            AppMsg::ProcessSyncedValue {
                height,
                round,
                proposer,
                value_bytes,
                reply,
            } => {
                let value = decode_value(value_bytes);

                if reply
                    .send(ProposedValue {
                        height,
                        round,
                        valid_round: Round::Nil,
                        proposer,
                        value,
                        validity: Validity::Valid,
                        extension: None,
                    })
                    .is_err()
                {
                    error!("Failed to send ProcessSyncedValue reply");
                }
            }

            AppMsg::RestreamProposal { .. } => {
                unimplemented!("RestreamValue");
            }
        }
    }

    Err(eyre!("Consensus channel closed unexpectedly"))
}
