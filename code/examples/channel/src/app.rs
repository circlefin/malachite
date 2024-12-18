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
            // The first message to handle is the `ConsensusReady` message, signaling to the app
            // that Malachite is ready to start consensus
            AppMsg::ConsensusReady { reply } => {
                info!("Consensus is ready");

                // We can simply respond by telling the engine to start consensus
                // at the current height, which is initially 1
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

            // The next message to handle is the `StartRound` message, signaling to the app
            // that consensus has entered a new round (including the initial round 0)
            AppMsg::StartedRound {
                height,
                round,
                proposer,
            } => {
                info!(%height, %round, %proposer, "Started round");

                // We can use that opportunity to update our internal state
                state.current_height = height;
                state.current_round = round;
                state.current_proposer = Some(proposer);
            }

            // At some point, we may end up being the proposer for that round, and the engine
            // will then ask us for a value to propose to the other validators.
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

                // Here it is important that, if we have previously built a value for this height and round,
                // we send back the very same value. We will not go into details here but this has to do
                // with crash recovery and is not strictly necessary in this example app since all our state
                // is kept in-memory and therefore is not crash tolerant at all.
                if let Some(proposal) = state.get_previously_built_value(height, round) {
                    info!(value = %proposal.value.id(), "Re-using previously built value");

                    if reply.send(proposal).is_err() {
                        error!("Failed to send GetValue reply");
                    }

                    return Ok(());
                }

                // If we have not previously built a value for that very same height and round,
                // we need to create a new value to propose and send it back to consensus.
                let proposal = state.propose_value(height, round);

                // Send it to consensus
                if reply.send(proposal.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                // Now what's left to do is to break down the value to propose into parts,
                // and send those parts over the network to our peers, for them to re-assemble the full value.
                for stream_message in state.stream_proposal(proposal) {
                    info!(%height, %round, "Streaming proposal part: {stream_message:?}");
                    channels
                        .network
                        .send(NetworkMsg::PublishProposalPart(stream_message))
                        .await?;
                }

                // NOTE: In this tutorial, the value is simply an integer and therefore results in a very small
                // message to gossip over the network, but if we were building a real application,
                // say building blocks containing thousands of transactions, the proposal would typically only
                // carry the block hash and the full block itself would be split into parts in order to
                // avoid blowing up the bandwidth requirements by gossiping a single huge message.
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

    // If we get there, it can only be because the channel we use to receive message
    // from consensus has been closed, meaning that the consensus actor has died.
    // We can do nothing but return an error here.
    Err(eyre!("Consensus channel closed unexpectedly"))
}
