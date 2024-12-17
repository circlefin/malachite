use eyre::eyre;
use tracing::{debug, error};

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
            AppMsg::ConsensusReady { reply_to } => {
                debug!("Consensus is ready");

                if reply_to
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
                state.current_height = height;
                state.current_round = round;
                state.current_proposer = Some(proposer);
            }

            AppMsg::GetValue {
                height,
                round: _,
                timeout_duration: _,
                address: _,
                reply_to,
            } => {
                let proposal = state.propose_value(&height);

                let value = LocallyProposedValue::new(
                    proposal.height,
                    proposal.round,
                    proposal.value,
                    proposal.extension,
                );

                // Send it to consensus
                if reply_to.send(value.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                let stream_message = state.create_broadcast_message(value);

                // Broadcast it to others. Old messages need not be broadcast.
                channels
                    .network
                    .send(NetworkMsg::PublishProposalPart(stream_message))
                    .await?;
            }

            AppMsg::GetEarliestBlockHeight { reply_to } => {
                if reply_to.send(state.get_earliest_height()).is_err() {
                    error!("Failed to send GetEarliestBlockHeight reply");
                }
            }

            AppMsg::ReceivedProposalPart {
                from: _,
                part,
                reply_to,
            } => {
                if let Some(proposed_value) = state.add_proposal(part) {
                    if reply_to.send(proposed_value).is_err() {
                        error!("Failed to send ReceivedProposalPart reply");
                    }
                }
            }

            AppMsg::GetValidatorSet {
                height: _,
                reply_to,
            } => {
                if reply_to.send(genesis.validator_set.clone()).is_err() {
                    error!("Failed to send GetValidatorSet reply");
                }
            }

            AppMsg::Decided {
                certificate,
                reply_to,
            } => {
                state.commit_block(certificate);
                if reply_to
                    .send(ConsensusMsg::StartHeight(
                        state.current_height,
                        genesis.validator_set.clone(),
                    ))
                    .is_err()
                {
                    error!("Failed to send Decided reply");
                }
            }

            AppMsg::GetDecidedBlock { height, reply_to } => {
                let block = state.get_block(&height).cloned();
                if reply_to.send(block).is_err() {
                    error!("Failed to send GetDecidedBlock reply");
                }
            }

            AppMsg::ProcessSyncedValue {
                height,
                round,
                validator_address,
                value_bytes,
                reply_to,
            } => {
                let value = decode_value(value_bytes);

                if reply_to
                    .send(ProposedValue {
                        height,
                        round,
                        valid_round: Round::Nil,
                        validator_address,
                        value,
                        validity: Validity::Valid,
                        extension: None,
                    })
                    .is_err()
                {
                    error!("Failed to send ProcessSyncedBlock reply");
                }
            }

            AppMsg::RestreamValue { .. } => {
                unimplemented!("RestreamValue");
            }
        }
    }

    Err(eyre!("Consensus channel closed unexpectedly"))
}
