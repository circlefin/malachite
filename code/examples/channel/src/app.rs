use std::time::Duration;

use eyre::eyre;
use tracing::{error, info};

use malachitebft_app_channel::app::streaming::StreamContent;
use malachitebft_app_channel::app::types::core::{Round, Validity};
use malachitebft_app_channel::app::types::ProposedValue;
use malachitebft_app_channel::{AppMsg, Channels, ConsensusMsg, NetworkMsg};
use malachitebft_test::{Genesis, TestContext};

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

                tokio::time::sleep(Duration::from_secs(1)).await;

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
                // NOTE: We can ignore the timeout as we are building the value right away.
                // If we were let's say reaping as many txes from a mempool and executing them,
                // then we would need to respect the timeout and stop at a certain point.

                info!(%height, %round, "Consensus is requesting a value to propose");

                // Check if we have a previously built value for that height and round
                if let Some(proposal) = state.get_previously_built_value(height, round) {
                    info!(value = %proposal.value.id(), "Re-using previously built value");

                    if reply.send(proposal).is_err() {
                        error!("Failed to send GetValue reply");
                    }

                    return Ok(());
                }

                // Otherwise, propose a new value
                let proposal = state.propose_value(height, round);

                // Send it to consensus
                if reply.send(proposal.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                // Decompose the proposal into proposal parts and stream them over the network
                for stream_message in state.stream_proposal(proposal) {
                    info!(%height, %round, "Streaming proposal part: {stream_message:?}");
                    channels
                        .network
                        .send(NetworkMsg::PublishProposalPart(stream_message))
                        .await?;
                }
            }

            AppMsg::GetHistoryMinHeight { reply } => {
                if reply.send(state.get_earliest_height()).is_err() {
                    error!("Failed to send GetHistoryMinHeight reply");
                }
            }

            AppMsg::ReceivedProposalPart { from, part, reply } => {
                let part_type = match &part.content {
                    StreamContent::Data(part) => part.get_type(),
                    StreamContent::Fin(_) => "end of stream",
                };

                info!(%from, %part.sequence, part.type = %part_type, "Received proposal part");

                let proposed_value = state.received_proposal_part(from, part);

                if reply.send(proposed_value).is_err() {
                    error!("Failed to send ReceivedProposalPart reply");
                }
            }

            AppMsg::GetValidatorSet { height: _, reply } => {
                if reply.send(genesis.validator_set.clone()).is_err() {
                    error!("Failed to send GetValidatorSet reply");
                }
            }

            AppMsg::Decided { certificate, reply } => {
                info!(
                    height = %certificate.height, round = %certificate.round,
                    value = %certificate.value_id,
                    "Consensus has decided on value"
                );

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
                info!(%height, %round, "Processing synced value");

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
                error!("RestreamProposal not implemented");
            }
        }
    }

    Err(eyre!("Consensus channel closed unexpectedly"))
}
