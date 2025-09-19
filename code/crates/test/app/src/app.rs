use std::time::Duration;

use eyre::eyre;
use malachitebft_app_channel::app::engine::host::Next;
use tokio::time::sleep;
use tracing::{debug, error, info};

// use malachitebft_app_channel::app::config::ValuePayload;
use malachitebft_app_channel::app::streaming::StreamContent;
use malachitebft_app_channel::app::types::codec::Codec;
use malachitebft_app_channel::app::types::core::{Round, Validity};
use malachitebft_app_channel::app::types::sync::RawDecidedValue;
use malachitebft_app_channel::app::types::{LocallyProposedValue, ProposedValue};
use malachitebft_app_channel::{AppMsg, Channels, NetworkMsg};
use malachitebft_test::codec::json::JsonCodec;
use malachitebft_test::{Height, TestContext};

use crate::state::{decode_value, State};

pub async fn run(state: &mut State, channels: &mut Channels<TestContext>) -> eyre::Result<()> {
    while let Some(msg) = channels.consensus.recv().await {
        match msg {
            // The first message to handle is the `ConsensusReady` message, signaling to the app
            // that Malachite is ready to start consensus
            AppMsg::ConsensusReady { reply } => {
                let start_height = state
                    .store
                    .max_decided_value_height()
                    .await
                    .map(|height| height.increment())
                    .unwrap_or_else(|| Height::new(1));

                info!(%start_height, "Consensus is ready");

                sleep(Duration::from_millis(200)).await;

                // We can simply respond by telling the engine to start consensus
                // at the next height, and provide it with the appropriate validator set
                let validator_set = state
                    .get_validator_set(start_height)
                    .expect("Validator set should be available");

                if reply.send((start_height, validator_set)).is_err() {
                    error!("Failed to send ConsensusReady reply");
                }
            }

            // The next message to handle is the `StartRound` message, signaling to the app
            // that consensus has entered a new round (including the initial round 0)
            AppMsg::StartedRound {
                height,
                round,
                proposer,
                role,
                reply_value,
            } => {
                info!(%height, %round, %proposer, ?role, "Started round");

                // We can use that opportunity to update our internal state
                state.current_height = height;
                state.current_round = round;
                state.current_proposer = Some(proposer);

                let pending_parts = state
                    .store
                    .get_pending_proposal_parts(height, round)
                    .await?;
                info!(%height, %round, "Found {} pending proposal parts, validating...", pending_parts.len());

                for parts in &pending_parts {
                    // Remove the parts from pending
                    state
                        .store
                        .remove_pending_proposal_parts(parts.clone())
                        .await?;

                    match state.validate_proposal_parts(parts) {
                        Ok(()) => {
                            // Validation passed - convert to ProposedValue and move to undecided
                            let value = State::assemble_value_from_parts(parts.clone())?;
                            state.store.store_undecided_proposal(value).await?;
                            info!(
                                height = %parts.height,
                                round = %parts.round,
                                proposer = %parts.proposer,
                                "Moved valid pending proposal to undecided after validation"
                            );
                        }
                        Err(error) => {
                            // Validation failed, log error
                            error!(
                                height = %parts.height,
                                round = %parts.round,
                                proposer = %parts.proposer,
                                error = ?error,
                                "Removed invalid pending proposal"
                            );
                        }
                    }
                }

                // If we have already built or seen values for this height and round,
                // send them back to consensus. This may happen when we are restarting after a crash.
                let proposals = state.store.get_undecided_proposals(height, round).await?;
                info!(%height, %round, "Found {} undecided proposals", proposals.len());

                if reply_value.send(proposals).is_err() {
                    error!("Failed to send undecided proposals");
                }
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
                tracing::debug!(%height, %round, "Middleware: {:?}", state.ctx.middleware());

                // Here it is important that, if we have previously built a value for this height and round,
                // we send back the very same value.
                let proposal = match state.get_previously_built_value(height, round).await? {
                    Some(mut proposal) => {
                        state
                            .ctx
                            .middleware()
                            .on_propose_value(&state.ctx, &mut proposal, true);

                        proposal
                    }
                    None => {
                        // If we have not previously built a value for that very same height and round,
                        // we need to create a new value to propose and send it back to consensus.
                        let mut proposal = state.propose_value(height, round).await?;

                        state
                            .ctx
                            .middleware()
                            .on_propose_value(&state.ctx, &mut proposal, false);

                        proposal
                    }
                };

                // Send it to consensus
                if reply.send(proposal.clone()).is_err() {
                    error!("Failed to send GetValue reply");
                }

                // The POL round is always nil when we propose a newly built value.
                // See L15/L18 of the Tendermint algorithm.
                let pol_round = Round::Nil;

                // Now what's left to do is to break down the value to propose into parts,
                // and send those parts over the network to our peers, for them to re-assemble the full value.
                for stream_message in state.stream_proposal(proposal, pol_round) {
                    debug!(%height, %round, "Streaming proposal part: {stream_message:?}");

                    channels
                        .network
                        .send(NetworkMsg::PublishProposalPart(stream_message))
                        .await?;
                }
            }

            // On the receiving end of these proposal parts (ie. when we are not the proposer),
            // we need to process these parts and re-assemble the full value.
            // To this end, we store each part that we receive and assemble the full value once we
            // have all its constituent parts. Then we send that value back to consensus for it to
            // consider and vote for or against it (ie. vote `nil`), depending on its validity.
            AppMsg::ReceivedProposalPart { from, part, reply } => {
                let part_type = match &part.content {
                    StreamContent::Data(part) => part.get_type(),
                    StreamContent::Fin => "end of stream",
                };

                debug!(%from, %part.sequence, part.type = %part_type, "Received proposal part");

                let proposed_value = state.received_proposal_part(from, part).await?;

                if reply.send(proposed_value).is_err() {
                    error!("Failed to send ReceivedProposalPart reply");
                }
            }

            // After some time, consensus will finally reach a decision on the value
            // to commit for the current height, and will notify the application,
            // providing it with a commit certificate which contains the ID of the value
            // that was decided on as well as the set of commits for that value,
            // ie. the precommits together with their (aggregated) signatures.
            AppMsg::Decided {
                certificate,
                extensions: _,
                reply,
            } => {
                info!(
                    height = %certificate.height,
                    round = %certificate.round,
                    value = %certificate.value_id,
                    "Consensus has decided on value, committing..."
                );
                assert!(!certificate.commit_signatures.is_empty());

                // When that happens, we store the decided value in our store
                match state.commit(certificate).await {
                    Ok(_) => {
                        // And then we instruct consensus to start the next height
                        let validator_set = state
                            .get_validator_set(state.current_height)
                            .expect("Validator set should be available");

                        if reply
                            .send(Next::Start(state.current_height, validator_set))
                            .is_err()
                        {
                            error!("Failed to send StartHeight reply");
                        }
                    }
                    Err(e) => {
                        // Commit failed, restart the height
                        error!("Commit failed: {e}");
                        error!("Restarting height {}", state.current_height);

                        let validator_set = state
                            .get_validator_set(state.current_height)
                            .expect("Validator set should be available");

                        if reply
                            .send(Next::Restart(state.current_height, validator_set))
                            .is_err()
                        {
                            error!("Failed to send RestartHeight reply");
                        }
                    }
                }
                sleep(Duration::from_millis(500)).await;
            }

            // It may happen that our node is lagging behind its peers. In that case,
            // a synchronization mechanism will automatically kick to try and catch up to
            // our peers. When that happens, some of these peers will send us decided values
            // for the current height only (not for future heights). When the engine receives
            // such a value, it will forward to the application to decode it from its wire format
            // and send back the decoded value to consensus.
            AppMsg::ProcessSyncedValue {
                height,
                round,
                proposer,
                value_bytes,
                reply,
            } => {
                info!(%height, %round, "Processing synced value");

                if let Some(value) = decode_value(value_bytes) {
                    let proposal = ProposedValue {
                        height,
                        round,
                        valid_round: Round::Nil,
                        proposer,
                        value,
                        validity: Validity::Valid,
                    };

                    // TODO: We plan to add some validation here in the future.
                    state.store_synced_value(proposal.clone()).await?;

                    if reply.send(Some(proposal)).is_err() {
                        error!("Failed to send ProcessSyncedValue reply");
                    }
                } else {
                    error!(%height, %round, "Failed to decode synced value");
                    if reply.send(None).is_err() {
                        error!("Failed to send ProcessSyncedValue reply");
                    }
                }
            }

            // If, on the other hand, we are not lagging behind but are instead asked by one of
            // our peer to help them catch up because they are the one lagging behind,
            // then the engine might ask the application to provide with the value
            // that was decided at some lower height. In that case, we fetch it from our store
            // and send it to consensus.
            AppMsg::GetDecidedValue { height, reply } => {
                info!(%height, "Received sync request for decided value");

                let decided_value = state.get_decided_value(height).await;
                info!(%height, "Found decided value: {decided_value:?}");

                let raw_decided_value = decided_value.map(|decided_value| RawDecidedValue {
                    certificate: decided_value.certificate,
                    value_bytes: JsonCodec.encode(&decided_value.value).unwrap(), // FIXME: unwrap
                });

                if reply.send(raw_decided_value).is_err() {
                    error!("Failed to send GetDecidedValue reply");
                }
            }

            // In order to figure out if we can help a peer that is lagging behind,
            // the engine may ask us for the height of the earliest available value in our store.
            AppMsg::GetHistoryMinHeight { reply } => {
                let min_height = state.get_earliest_height().await;

                if reply.send(min_height).is_err() {
                    error!("Failed to send GetHistoryMinHeight reply");
                }
            }

            AppMsg::RestreamProposal {
                height,
                round,
                valid_round,
                address: _,
                value_id,
            } => {
                info!(%height, %valid_round, "Restreaming existing proposal...");

                //  Look for a proposal at valid_round or round(should be already stored)
                let proposal_round = if valid_round == Round::Nil {
                    round
                } else {
                    valid_round
                };
                //  Look for a proposal for the given value_id at valid_round (should be already stored)
                let proposal = state
                    .store
                    .get_undecided_proposal(height, proposal_round, value_id)
                    .await?;

                if let Some(proposal) = proposal {
                    assert_eq!(proposal.value.id(), value_id);

                    let locally_proposed_value = LocallyProposedValue {
                        height,
                        round,
                        value: proposal.value,
                    };

                    for stream_message in state.stream_proposal(locally_proposed_value, valid_round)
                    {
                        debug!(%height, %valid_round, "Publishing proposal part: {stream_message:?}");

                        channels
                            .network
                            .send(NetworkMsg::PublishProposalPart(stream_message))
                            .await?;
                    }
                }
            }

            AppMsg::ExtendVote { reply, .. } => {
                if reply.send(None).is_err() {
                    error!("Failed to send ExtendVote reply");
                }
            }

            AppMsg::VerifyVoteExtension { reply, .. } => {
                if reply.send(Ok(())).is_err() {
                    error!("Failed to send VerifyVoteExtension reply");
                }
            }
        }
    }

    // If we get there, it can only be because the channel we use to receive message
    // from consensus has been closed, meaning that the consensus actor has died.
    // We can do nothing but return an error here.
    Err(eyre!("Consensus channel closed unexpectedly"))
}
