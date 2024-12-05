use std::path::PathBuf;

use eyre::eyre;
use itertools::Itertools;
use ractor::{async_trait, Actor, ActorProcessingErr, SpawnErr};
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::time::Instant;
use tracing::{debug, error, info, trace, warn};

use malachite_actors::consensus::ConsensusMsg;
use malachite_actors::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef};
use malachite_actors::host::{LocallyProposedValue, ProposedValue};
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_blocksync::SyncedBlock;
use malachite_common::{Round, Validity};
use malachite_metrics::Metrics;
use malachite_starknet_p2p_types::{Block, PartType};

use crate::host::proposal::compute_proposal_signature;
use crate::host::state::HostState;
use crate::host::{Host as _, StarknetHost};
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::proto::Protobuf;
use crate::types::*;

pub struct Host {
    mempool: MempoolRef,
    gossip_consensus: GossipConsensusRef<MockContext>,
    metrics: Metrics,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl Host {
    pub async fn spawn(
        home_dir: PathBuf,
        host: StarknetHost,
        mempool: MempoolRef,
        gossip_consensus: GossipConsensusRef<MockContext>,
        metrics: Metrics,
    ) -> Result<HostRef, SpawnErr> {
        let db_dir = home_dir.join("db");
        std::fs::create_dir_all(&db_dir).map_err(|e| SpawnErr::StartupFailed(e.into()))?;
        let db_path = db_dir.join("blocks.db");

        let (actor_ref, _) = Actor::spawn(
            None,
            Self::new(mempool, gossip_consensus, metrics),
            HostState::new(host, db_path, &mut StdRng::from_entropy()),
        )
        .await?;

        Ok(actor_ref)
    }

    pub fn new(
        mempool: MempoolRef,
        gossip_consensus: GossipConsensusRef<MockContext>,
        metrics: Metrics,
    ) -> Self {
        Self {
            mempool,
            gossip_consensus,
            metrics,
        }
    }

    async fn prune_block_store(&self, state: &mut HostState) {
        let max_height = state.block_store.last_height().unwrap_or_default();
        let max_retain_blocks = state.host.params.max_retain_blocks as u64;

        // Compute the height to retain blocks higher than
        let retain_height = max_height.as_u64().saturating_sub(max_retain_blocks);
        if retain_height <= 1 {
            // No need to prune anything, since we would retain every blocks
            return;
        }

        let retain_height = Height::new(retain_height, max_height.fork_id);
        match state.block_store.prune(retain_height).await {
            Ok(pruned) => {
                debug!(
                    %retain_height, pruned_heights = pruned.iter().join(", "),
                    "Pruned the block store"
                );
            }
            Err(e) => {
                error!(%e, %retain_height, "Failed to prune the block store");
            }
        }
    }
}

#[async_trait]
impl Actor for Host {
    type Arguments = HostState;
    type State = HostState;
    type Msg = HostMsg;

    async fn pre_start(
        &self,
        _myself: HostRef,
        initial_state: Self::State,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(initial_state)
    }

    async fn handle(
        &self,
        _myself: HostRef,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::ConsensusReady(consensus) => {
                let latest_block_height = state.block_store.last_height().unwrap_or_default();
                let start_height = latest_block_height.increment();

                consensus.cast(ConsensusMsg::StartHeight(
                    start_height,
                    state.host.validator_set.clone(),
                ))?;

                Ok(())
            }

            HostMsg::StartedRound {
                height,
                round,
                proposer,
            } => {
                state.height = height;
                state.round = round;
                state.proposer = Some(proposer);

                Ok(())
            }

            HostMsg::GetEarliestBlockHeight { reply_to } => {
                let earliest_block_height = state.block_store.first_height().unwrap_or_default();
                reply_to.send(earliest_block_height)?;
                Ok(())
            }

            HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                address: _,
                reply_to,
            } => {
                // If we have already built a block for this height and round, return it
                // This may happen when we are restarting after a crash and replaying the WAL.
                if let Some(block) = state.block_store.get_undecided_block(height, round).await? {
                    info!(%height, %round, hash = %block.block_hash, "Returning previously built block");

                    let value = LocallyProposedValue::new(height, round, block.block_hash, None);
                    reply_to.send(value)?;

                    return Ok(());
                }

                let deadline = Instant::now() + timeout_duration;

                debug!(%height, %round, "Building new proposal...");

                let (mut rx_part, rx_hash) =
                    state.host.build_new_proposal(height, round, deadline).await;

                let stream_id = state.next_stream_id();

                let mut sequence = 0;

                while let Some(part) = rx_part.recv().await {
                    state.host.part_store.store(height, round, part.clone());

                    if state.host.params.value_payload.include_parts() {
                        debug!(%stream_id, %sequence, "Broadcasting proposal part");

                        let msg = StreamMessage::new(
                            stream_id,
                            sequence,
                            StreamContent::Data(part.clone()),
                        );

                        self.gossip_consensus
                            .cast(GossipConsensusMsg::PublishProposalPart(msg))?;
                    }

                    sequence += 1;
                }

                if state.host.params.value_payload.include_parts() {
                    let msg = StreamMessage::new(stream_id, sequence, StreamContent::Fin(true));

                    self.gossip_consensus
                        .cast(GossipConsensusMsg::PublishProposalPart(msg))?;
                }

                let block_hash = rx_hash.await?;
                debug!(%block_hash, "Assembled block");

                state
                    .host
                    .part_store
                    .store_value_id(height, round, block_hash);

                let parts = state.host.part_store.all_parts(height, round);

                let Some((value, block)) =
                    state.build_block_from_parts(&parts, height, round).await
                else {
                    error!(%height, %round, "Failed to build block from parts");
                    return Ok(());
                };

                if let Err(e) = state
                    .block_store
                    .store_undecided_block(value.height, value.round, block)
                    .await
                {
                    error!(%e, %height, %round, "Failed to store the proposed block");
                }

                reply_to.send(LocallyProposedValue::new(
                    value.height,
                    value.round,
                    value.value,
                    value.extension,
                ))?;

                Ok(())
            }

            HostMsg::RestreamValue {
                height,
                round,
                valid_round,
                address,
                value_id,
            } => {
                debug!(%height, %round, "Restreaming existing proposal...");

                let mut rx_part = state.host.send_known_proposal(value_id).await;

                let stream_id = state.next_stream_id();

                let init = ProposalInit {
                    height,
                    proposal_round: round,
                    valid_round,
                    proposer: address.clone(),
                };

                let signature =
                    compute_proposal_signature(&init, &value_id, &state.host.private_key);

                let init_part = ProposalPart::Init(init);
                let fin_part = ProposalPart::Fin(ProposalFin { signature });

                debug!(%height, %round, "Created new Init part: {init_part:?}");

                let mut sequence = 0;

                while let Some(part) = rx_part.recv().await {
                    let new_part = match part.part_type() {
                        PartType::Init => init_part.clone(),
                        PartType::Fin => fin_part.clone(),
                        PartType::Transactions | PartType::BlockProof => part,
                    };

                    state.host.part_store.store(height, round, new_part.clone());

                    if state.host.params.value_payload.include_parts() {
                        debug!(%stream_id, %sequence, "Broadcasting proposal part");

                        let msg =
                            StreamMessage::new(stream_id, sequence, StreamContent::Data(new_part));

                        self.gossip_consensus
                            .cast(GossipConsensusMsg::PublishProposalPart(msg))?;

                        sequence += 1;
                    }
                }

                Ok(())
            }

            HostMsg::ReceivedProposalPart {
                from,
                part,
                reply_to,
            } => {
                // TODO - use state.host.receive_proposal() and move some of the logic below there
                let sequence = part.sequence;

                let Some(parts) = state.part_streams_map.insert(from, part) else {
                    return Ok(());
                };

                if parts.height < state.height {
                    trace!(
                        height = %state.height,
                        round = %state.round,
                        part.height = %parts.height,
                        part.round = %parts.round,
                        part.sequence = %sequence,
                        "Received outdated proposal part, ignoring"
                    );

                    return Ok(());
                }

                for part in parts.parts {
                    debug!(
                        part.sequence = %sequence,
                        part.height = %parts.height,
                        part.round = %parts.round,
                        part.message = ?part.part_type(),
                        "Processing proposal part"
                    );

                    if let Some(value) = state
                        .build_value_from_part(parts.height, parts.round, part)
                        .await
                    {
                        reply_to.send(value)?;
                        break;
                    }
                }

                Ok(())
            }

            HostMsg::GetValidatorSet { height, reply_to } => {
                if let Some(validators) = state.host.validators(height).await {
                    reply_to.send(ValidatorSet::new(validators))?;
                    Ok(())
                } else {
                    Err(eyre!("No validator set found for the given height {height}").into())
                }
            }

            HostMsg::Decided {
                certificate,
                consensus,
            } => {
                let (height, round) = (certificate.height, certificate.round);

                let mut all_parts = state.host.part_store.all_parts(height, round);

                let mut all_txes = vec![];
                for part in all_parts.iter_mut() {
                    if let ProposalPart::Transactions(transactions) = part.as_ref() {
                        let mut txes = transactions.to_vec();
                        all_txes.append(&mut txes);
                    }
                }

                // Build the block from transaction parts and certificate, and store it
                if let Err(e) = state
                    .block_store
                    .store_decided_block(&certificate, &all_txes)
                    .await
                {
                    error!(%e, %height, %round, "Failed to store the block");
                }

                // Update metrics
                let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
                let extension_size: usize = certificate
                    .aggregated_signature
                    .signatures
                    .iter()
                    .map(|c| c.extension.as_ref().map(|e| e.size_bytes()).unwrap_or(0))
                    .sum();

                let block_and_commits_size = block_size + extension_size;
                let tx_count: usize = all_parts.iter().map(|p| p.tx_count()).sum();

                self.metrics.block_tx_count.observe(tx_count as f64);
                self.metrics
                    .block_size_bytes
                    .observe(block_and_commits_size as f64);
                self.metrics.finalized_txes.inc_by(tx_count as u64);

                // Gather hashes of all the tx-es included in the block,
                // so that we can notify the mempool to remove them.
                let mut tx_hashes = vec![];
                for part in all_parts {
                    if let ProposalPart::Transactions(txes) = &part.as_ref() {
                        tx_hashes.extend(txes.as_slice().iter().map(|tx| tx.hash()));
                    }
                }

                // Prune the PartStore of all parts for heights lower than `state.height`
                state.host.part_store.prune(state.height);

                // Store the block
                self.prune_block_store(state).await;

                // Notify the mempool to remove corresponding txs
                self.mempool.cast(MempoolMsg::Update { tx_hashes })?;

                // Notify Starknet Host of the decision
                state.host.decision(certificate).await;

                // Start the next height
                consensus.cast(ConsensusMsg::StartHeight(
                    state.height.increment(),
                    state.host.validator_set.clone(),
                ))?;

                Ok(())
            }

            HostMsg::GetDecidedBlock { height, reply_to } => {
                debug!(%height, "Received request for block");

                match state.block_store.get(height).await {
                    Ok(None) => {
                        let min = state.block_store.first_height().unwrap_or_default();
                        let max = state.block_store.last_height().unwrap_or_default();

                        warn!(%height, "No block for this height, available blocks: {min}..={max}");

                        reply_to.send(None)?;
                    }

                    Ok(Some(block)) => {
                        let block = SyncedBlock {
                            block_bytes: block.block.to_bytes().unwrap(),
                            certificate: block.certificate,
                        };

                        debug!(%height, "Found decided block in store");
                        reply_to.send(Some(block))?;
                    }
                    Err(e) => {
                        error!(%e, %height, "Failed to get decided block");
                        reply_to.send(None)?;
                    }
                }

                Ok(())
            }

            HostMsg::ProcessSyncedBlock {
                height,
                round,
                validator_address,
                block_bytes,
                reply_to,
            } => {
                let maybe_block = Block::from_bytes(block_bytes.as_ref());
                if let Ok(block) = maybe_block {
                    let proposed_value = ProposedValue {
                        height,
                        round,
                        valid_round: Round::Nil,
                        validator_address,
                        value: block.block_hash,
                        validity: Validity::Valid,
                        extension: None,
                    };

                    reply_to.send(proposed_value)?;
                }

                Ok(())
            }
        }
    }
}
