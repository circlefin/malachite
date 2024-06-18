#![allow(unused_variables)]

use std::sync::Arc;

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr, SpawnErr};
use sha2::Digest;
use tokio::time::Instant;
use tracing::{debug, error, trace};

use malachite_actors::consensus::{ConsensusMsg, Metrics};
use malachite_actors::host::{LocallyProposedValue, ReceivedProposedValue};
use malachite_actors::mempool::{MempoolMsg, MempoolRef};
use malachite_common::{Round, Validity};

use crate::hash::BlockHash;
use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::mock::part_store::PartStore;
use crate::mock::types::{Address, BlockPart, Height, ProposalContent, ProposalPart, ValidatorSet};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
    mempool: MempoolRef,
    metrics: Metrics,
}

#[derive(Default)]
pub struct HostState {
    part_store: PartStore<MockContext>,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl StarknetHost {
    pub fn new(host: MockHost, mempool: MempoolRef, metrics: Metrics) -> Self {
        Self {
            host,
            mempool,
            metrics,
        }
    }

    pub async fn spawn(
        host: MockHost,
        mempool: MempoolRef,
        metrics: Metrics,
    ) -> Result<HostRef, SpawnErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self::new(host, mempool, metrics),
            HostState::default(),
        )
        .await?;

        Ok(actor_ref)
    }

    pub fn build_proposal_content(
        &self,
        block_parts: &[Arc<BlockPart>],
        height: Height,
        round: Round,
    ) -> Option<(ProposalContent, Address, Validity)> {
        if block_parts.is_empty() {
            return None;
        }

        let mut block_hasher = sha2::Sha256::new();
        for block_part in block_parts {
            if let ProposalPart::TxBatch(_, tx_batch) = block_part.part.as_ref() {
                // Compute the expected block hash/value from the block parts.
                tx_batch.transactions().iter().for_each(|tx| {
                    block_hasher.update(tx.as_bytes());
                });
            }
        }

        let last_part = block_parts.last().expect("block_parts is not empty");

        let Some(metadata) = last_part.metadata() else {
            error!("Block is missing metadata");
            return None;
        };

        let received_value = metadata.hash;
        let expected_value = BlockHash::new(block_hasher.finalize().into());

        let valid = if received_value != expected_value {
            error!("Invalid block received with value {received_value}, expected {expected_value}");
            Validity::Invalid
        } else {
            Validity::Valid
        };

        let content = ProposalContent::new(metadata.clone());
        Some((content, last_part.validator_address, valid))
    }

    pub fn build_value(
        &self,
        block_parts: &[Arc<BlockPart>],
        height: Height,
        round: Round,
    ) -> Option<ReceivedProposedValue<MockContext>> {
        let (value, validator_address, valid) =
            self.build_proposal_content(block_parts, height, round)?;

        Some(ReceivedProposedValue {
            validator_address,
            height,
            round,
            value,
            valid,
        })
    }

    async fn build_value_from_block_part(
        &self,
        state: &mut HostState,
        block_part: BlockPart,
    ) -> Option<ReceivedProposedValue<MockContext>> {
        let height = block_part.height;
        let round = block_part.round;
        let sequence = block_part.sequence;

        trace!(%height, %round, %sequence, "Received block part");

        // Prune all block parts for heights lower than `height - 1`
        state.part_store.prune(height.decrement().unwrap_or(height));
        state.part_store.store(block_part.clone());

        // Simulate Tx execution and proof verification (assumes success)
        // TODO: Add config knob for invalid blocks
        let num_txes = block_part.tx_count().unwrap_or(0) as u32;
        tokio::time::sleep(self.host.params().exec_time_per_tx * num_txes).await;

        // Get the "last" part, the one with highest sequence.
        // Block parts may not be received in order.
        let all_parts = state.part_store.all_parts(height, round);
        let last_part = all_parts.last().expect("all_parts is not empty");

        // Check if the part with the highest sequence number had metadata content.
        // Otherwise abort, and wait for this part to be received.
        // TODO: Do more validations, e.g. there is no higher tx block part.
        let meta = last_part.metadata()?;

        let num_parts = all_parts.len();
        if num_parts == last_part.sequence as usize {
            let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
            let tx_count: usize = all_parts.iter().map(|p| p.tx_count().unwrap_or(0)).sum();

            debug!(
                %height,
                %round,
                %tx_count,
                %block_size,
                %num_parts,
                "Received last block part",
            );

            self.build_value(&all_parts, height, round)
        } else {
            None
        }
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

    #[tracing::instrument(name = "host", skip_all)]
    async fn handle(
        &self,
        _myself: HostRef,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                consensus,
                address,
                reply_to,
            } => {
                let deadline = Instant::now() + timeout_duration;

                let (mut rx_part, rx_hash) =
                    self.host.build_new_proposal(height, round, deadline).await;

                while let Some(part) = rx_part.recv().await {
                    let block_part = BlockPart::new(height, round, part.sequence(), address, part);
                    state.part_store.store(block_part.clone());

                    consensus.cast(ConsensusMsg::GossipBlockPart(block_part))?;
                }

                let block_hash = rx_hash.await?;
                debug!("Got block with hash: {block_hash}");

                let block_parts = state.part_store.all_parts(height, round);
                if let Some((value, ..)) = self.build_proposal_content(&block_parts, height, round)
                {
                    let proposed_value = LocallyProposedValue::new(height, round, value);
                    reply_to.send(proposed_value)?;
                }

                Ok(())
            }

            HostMsg::ReceivedBlockPart {
                block_part,
                reply_to,
            } => {
                if let Some(value) = self.build_value_from_block_part(state, block_part).await {
                    reply_to.send(value)?;
                }

                Ok(())
            }

            HostMsg::GetReceivedValue {
                height,
                round,
                reply_to,
            } => {
                let block_parts = state.part_store.all_parts(height, round);
                let proposed_value = self.build_value(&block_parts, height, round);
                reply_to.send(proposed_value)?;

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

            HostMsg::DecidedOnValue {
                height,
                round,
                value,
                commits,
            } => {
                let all_parts = state.part_store.all_parts(height, round);

                // TODO: Build the block from block parts and commits and store it

                // Update metrics
                let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
                let tx_count: usize = all_parts.iter().map(|p| p.tx_count().unwrap_or(0)).sum();

                self.metrics.block_tx_count.observe(tx_count as f64);
                self.metrics.block_size_bytes.observe(block_size as f64);
                self.metrics.finalized_txes.inc_by(tx_count as u64);

                // Send Update to mempool to remove all the tx-es included in the block.
                let mut tx_hashes = vec![];

                for part in all_parts {
                    if let ProposalPart::TxBatch(_, tx_batch) = part.part.as_ref() {
                        tx_hashes.extend(tx_batch.transactions().iter().map(|tx| {
                            use std::hash::{Hash, Hasher};
                            let mut hash = std::hash::DefaultHasher::new();
                            tx.as_bytes().hash(&mut hash);
                            hash.finish()
                        }));
                    }
                }

                // Prune the PartStore of all parts for heights lower than `height - 1`
                state.part_store.prune(height.decrement().unwrap_or(height));

                // Notify the mempool to remove corresponding txs
                self.mempool.cast(MempoolMsg::Update { tx_hashes })?;

                // Notify Starknet Host of the decision
                self.host
                    .decision(value.block_hash(), commits, height)
                    .await;

                Ok(())
            }
        }
    }
}
