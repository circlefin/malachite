use std::sync::Arc;

use eyre::eyre;

use itertools::Itertools;
use ractor::{async_trait, Actor, ActorProcessingErr, SpawnErr};
use rand::RngCore;
use sha3::Digest;
use tokio::time::Instant;
use tracing::{debug, error, trace, warn};

use malachite_actors::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef};
use malachite_actors::util::streaming::{StreamContent, StreamId, StreamMessage};
use malachite_blocksync::SyncedBlock;

use malachite_actors::consensus::ConsensusMsg;
use malachite_actors::host::{LocallyProposedValue, ProposedValue};
use malachite_common::{Extension, Proposal, Round, Validity, Value};
use malachite_metrics::Metrics;
use malachite_proto::Protobuf;

use crate::block_store::BlockStore;
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::part_store::PartStore;
use crate::streaming::PartStreamsMap;
use crate::types::{Address, BlockHash, Height, ProposalPart, ValidatorSet};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
    mempool: MempoolRef,
    gossip_consensus: GossipConsensusRef<MockContext>,
    metrics: Metrics,
}

pub struct HostState {
    height: Height,
    round: Round,
    proposer: Option<Address>,
    block_store: BlockStore,
    part_store: PartStore<MockContext>,
    part_streams_map: PartStreamsMap,
    next_stream_id: StreamId,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            height: Height::new(0, 0),
            round: Round::Nil,
            proposer: None,
            block_store: BlockStore::default(),
            part_store: PartStore::default(),
            part_streams_map: PartStreamsMap::default(),
            next_stream_id: StreamId::default(),
        }
    }
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl StarknetHost {
    pub fn new(
        host: MockHost,
        mempool: MempoolRef,
        gossip_consensus: GossipConsensusRef<MockContext>,
        metrics: Metrics,
    ) -> Self {
        Self {
            host,
            mempool,
            gossip_consensus,
            metrics,
        }
    }

    pub async fn spawn(
        host: MockHost,
        mempool: MempoolRef,
        gossip_consensus: GossipConsensusRef<MockContext>,
        metrics: Metrics,
    ) -> Result<HostRef, SpawnErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self::new(host, mempool, gossip_consensus, metrics),
            HostState::default(),
        )
        .await?;

        Ok(actor_ref)
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub fn build_value_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<ProposedValue<MockContext>> {
        let (value, validator_address, validity, extension) =
            self.build_proposal_content_from_parts(parts, height, round)?;

        Some(ProposedValue {
            validator_address,
            height,
            round,
            value,
            validity,
            extension,
        })
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub fn build_proposal_content_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<(BlockHash, Address, Validity, Option<Extension>)> {
        if parts.is_empty() {
            return None;
        }

        let Some(init) = parts.iter().find_map(|part| part.as_init()) else {
            error!("No Init part found in the proposal parts");
            return None;
        };

        let Some(_fin) = parts.iter().find_map(|part| part.as_fin()) else {
            error!("No Fin part found in the proposal parts");
            return None;
        };

        trace!(parts.len = %parts.len(), "Building proposal content from parts");

        let extension = self.host.params().vote_extensions.enabled.then(|| {
            let size = self.host.params().vote_extensions.size.as_u64() as usize;
            debug!(%size, "Vote extensions are enabled" );

            let mut bytes = vec![0u8; size];
            rand::thread_rng().fill_bytes(&mut bytes);

            Extension::from(bytes)
        });

        let block_hash = {
            let mut block_hasher = sha3::Keccak256::new();
            for part in parts {
                block_hasher.update(part.to_sign_bytes());
            }
            BlockHash::new(block_hasher.finalize().into())
        };

        trace!(%block_hash, "Computed block hash");

        // TODO: How to compute validity?
        let validity = Validity::Valid;

        Some((block_hash, init.proposer.clone(), validity, extension))
    }

    #[tracing::instrument(skip_all, fields(
        part.height = %height,
        part.round = %round,
        part.message = ?part.part_type(),
    ))]
    async fn build_value_from_part(
        &self,
        state: &mut HostState,
        height: Height,
        round: Round,
        part: ProposalPart,
    ) -> Option<ProposedValue<MockContext>> {
        state.part_store.store(height, round, part.clone());

        if let ProposalPart::Transactions(_txes) = &part {
            debug!("Simulating tx execution and proof verification");

            // Simulate Tx execution and proof verification (assumes success)
            // TODO: Add config knob for invalid blocks
            let num_txes = part.tx_count() as u32;
            let exec_time = self.host.params().exec_time_per_tx * num_txes;
            tokio::time::sleep(exec_time).await;

            trace!("Simulation took {exec_time:?} to execute {num_txes} txes");
        }

        let all_parts = state.part_store.all_parts(height, round);

        trace!(
            count = state.part_store.blocks_stored(),
            "The store has blocks"
        );

        // TODO: Do more validations, e.g. there is no higher tx proposal part,
        //       check that we have received the proof, etc.
        let Some(_fin) = all_parts.iter().find_map(|part| part.as_fin()) else {
            debug!("Final proposal part has not been received yet");
            return None;
        };

        let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
        let tx_count: usize = all_parts.iter().map(|p| p.tx_count()).sum();

        debug!(
            %tx_count, %block_size, num_parts = %all_parts.len(),
            "All parts have been received already, building value"
        );

        self.build_value_from_parts(&all_parts, height, round)
    }

    fn store_block(&self, state: &mut HostState) {
        let max_height = state.block_store.store_keys().last().unwrap_or_default();

        let min_number_blocks: u64 = std::cmp::min(
            self.host.params().max_retain_blocks as u64,
            max_height.as_u64(),
        );

        let retain_height =
            Height::new(max_height.as_u64() - min_number_blocks, max_height.fork_id);

        state.block_store.prune(retain_height);
    }
}

#[async_trait]
impl Actor for StarknetHost {
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
            HostMsg::StartRound {
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
                let earliest_block_height =
                    state.block_store.store_keys().next().unwrap_or_default();
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
                let deadline = Instant::now() + timeout_duration;

                debug!(%height, %round, "Building new proposal...");

                let (mut rx_part, rx_hash) =
                    self.host.build_new_proposal(height, round, deadline).await;

                let stream_id = state.next_stream_id;
                state.next_stream_id += 1;

                let mut sequence = 0;

                while let Some(part) = rx_part.recv().await {
                    state.part_store.store(height, round, part.clone());

                    debug!(%stream_id, %sequence, "Broadcasting proposal part");

                    let msg =
                        StreamMessage::new(stream_id, sequence, StreamContent::Data(part.clone()));
                    sequence += 1;

                    self.gossip_consensus
                        .cast(GossipConsensusMsg::PublishProposalPart(msg))?;
                }

                let msg = StreamMessage::new(stream_id, sequence, StreamContent::Fin(true));

                self.gossip_consensus
                    .cast(GossipConsensusMsg::PublishProposalPart(msg))?;

                let block_hash = rx_hash.await?;
                debug!(%block_hash, "Assembled block");

                let parts = state.part_store.all_parts(height, round);

                let extension = self.host.params().vote_extensions.enabled.then(|| {
                    let size = self.host.params().vote_extensions.size.as_u64() as usize;
                    debug!(%size, "Vote extensions are enabled");

                    let mut bytes = vec![0u8; size];
                    rand::thread_rng().fill_bytes(&mut bytes);

                    Extension::from(bytes)
                });

                if let Some(value) = self.build_value_from_parts(&parts, height, round) {
                    reply_to.send(LocallyProposedValue::new(
                        value.height,
                        value.round,
                        value.value,
                        extension,
                    ))?;
                }

                Ok(())
            }

            HostMsg::ReceivedProposalPart {
                from,
                part,
                reply_to,
            } => {
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

                    if let Some(value) = self
                        .build_value_from_part(state, parts.height, parts.round, part)
                        .await
                    {
                        reply_to.send(value)?;
                        break;
                    }
                }

                Ok(())
            }

            HostMsg::GetValidatorSet { height, reply_to } => {
                if let Some(validators) = self.host.validators(height).await {
                    reply_to.send(ValidatorSet::new(validators))?;
                    Ok(())
                } else {
                    Err(eyre!("No validator set found for the given height {height}").into())
                }
            }

            HostMsg::Decide {
                proposal,
                commits,
                consensus,
            } => {
                let height = proposal.height;
                let round = proposal.round;
                let mut all_parts = state.part_store.all_parts(height, round);

                let mut all_txes = vec![];
                for arc in all_parts.iter_mut() {
                    let part = Arc::unwrap_or_clone((*arc).clone());
                    if let ProposalPart::Transactions(transactions) = part {
                        let mut txes = transactions.into_vec();
                        all_txes.append(&mut txes);
                    }
                }

                // Build the block from proposal parts and commits and store it
                state.block_store.store(&proposal, &all_txes, &commits);

                // Update metrics
                let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
                let extension_size: usize = commits
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
                state.part_store.prune(state.height);

                // Store the block
                self.store_block(state);

                // Notify the mempool to remove corresponding txs
                self.mempool.cast(MempoolMsg::Update { tx_hashes })?;

                // Notify Starknet Host of the decision
                self.host
                    .decision(proposal.block_hash, commits, height)
                    .await;

                // Start the next height
                consensus.cast(ConsensusMsg::StartHeight(state.height.increment()))?;

                Ok(())
            }

            HostMsg::GetDecidedBlock { height, reply_to } => {
                debug!(%height, "Received request for block");

                match state.block_store.store.get(&height).cloned() {
                    None => {
                        warn!(
                            %height,
                            "No block found, available blocks: {}",
                            state.block_store.store_keys().format(", ")
                        );

                        reply_to.send(None)?;
                    }
                    Some(block) => {
                        let block = SyncedBlock {
                            proposal: block.proposal,
                            block_bytes: block.block.to_bytes().unwrap(),
                            certificate: block.certificate,
                        };

                        debug!(%height, "Found decided block in store");
                        reply_to.send(Some(block))?;
                    }
                }

                Ok(())
            }

            HostMsg::ProcessSyncedBlockBytes {
                proposal,
                block_bytes,
                reply_to,
            } => {
                // TODO - process and check that block_bytes match the proposal
                let _block_hash = {
                    let mut block_hasher = sha3::Keccak256::new();
                    block_hasher.update(block_bytes);
                    BlockHash::new(block_hasher.finalize().into())
                };

                let proposal = ProposedValue {
                    height: proposal.height(),
                    round: proposal.round(),
                    validator_address: proposal.validator_address().clone(),
                    value: proposal.value().id(),
                    validity: Validity::Valid,
                    extension: None,
                };

                reply_to.send(proposal)?;

                Ok(())
            }
        }
    }
}
