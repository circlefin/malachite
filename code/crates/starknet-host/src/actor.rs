#![allow(unused_variables)]

use std::sync::Arc;

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr, SpawnErr};
use tokio::time::Instant;

use malachite_actors::consensus::{ConsensusMsg, Metrics};
use malachite_actors::host::{LocallyProposedValue, ReceivedProposedValue};
use malachite_common::{Round, TransactionBatch, Validity};
use tracing::{info, trace};

use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::mock::part_store::PartStore;
use crate::mock::types::{Address, BlockPart, Content, Height, ProposalPart, ValidatorSet};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
    metrics: Metrics,
}

#[derive(Default)]
pub struct HostState {
    part_store: PartStore<MockContext>,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl StarknetHost {
    pub fn new(host: MockHost, metrics: Metrics) -> Self {
        Self { host, metrics }
    }

    pub async fn spawn(host: MockHost, metrics: Metrics) -> Result<HostRef, SpawnErr> {
        let (actor_ref, _) =
            Actor::spawn(None, Self::new(host, metrics), HostState::default()).await?;

        Ok(actor_ref)
    }

    pub fn build_proposal_content(
        &self,
        block_parts: &[Arc<BlockPart>],
        height: Height,
        round: Round,
    ) -> Option<(Content, Address)> {
        if block_parts.is_empty() {
            return None;
        }

        let last_part = block_parts.last().expect("block_parts is not empty");

        let mut metadata = None;
        let mut tx_batches = Vec::new();

        for block_part in block_parts {
            match block_part.part.as_ref() {
                ProposalPart::TxBatch(_, tx_batch) => {
                    tx_batches.extend(tx_batch.transactions().iter().cloned())
                }
                ProposalPart::Metadata(_, meta) => metadata = Some(meta.clone()),
            }
        }

        metadata.map(|metadata| {
            (
                Content {
                    tx_batch: TransactionBatch::new(tx_batches),
                    metadata,
                },
                last_part.validator_address,
            )
        })
    }

    pub fn build_value(
        &self,
        block_parts: &[Arc<BlockPart>],
        height: Height,
        round: Round,
    ) -> Option<ReceivedProposedValue<MockContext>> {
        let (value, validator_address) = self.build_proposal_content(block_parts, height, round)?;

        Some(ReceivedProposedValue {
            validator_address,
            height,
            round,
            value,
            valid: Validity::Valid, // TODO: Check validity
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
        // TODO - add config knob for invalid blocks
        let num_txes = block_part.tx_count().unwrap_or(0) as u32;
        tokio::time::sleep(self.host.params().exec_time_per_tx * num_txes).await;

        // Get the "last" part, the one with highest sequence.
        // Block parts may not be received in order.
        let all_parts = state.part_store.all_parts(height, round);
        let last_part = all_parts.last().expect("all_parts is not empty");

        // If the "last" part includes a metadata then this is truly the last part.
        // So in this case all block parts have been received, including the metadata that includes
        // the block hash/ value. This can be returned as the block is complete.
        //
        // TODO: the logic here is weak, we assume earlier parts don't include metadata
        // Should change once we implement `oneof`/ proper enum in protobuf but good enough for now test code
        let meta = last_part.metadata()?;

        let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
        let tx_count: usize = all_parts.iter().map(|p| p.tx_count().unwrap_or(0)).sum();

        info!(
            %height,
            %round,
            %tx_count,
            %block_size,
            num_parts = %all_parts.len(),
            "Received last block part",
        );

        // FIXME: At this point we don't know if this block (and its txes) will be decided on.
        //        So these need to be moved after the block is decided.
        self.metrics.block_tx_count.observe(tx_count as f64);
        self.metrics.block_size_bytes.observe(block_size as f64);
        self.metrics.finalized_txes.inc_by(tx_count as u64);

        self.build_value(&all_parts, height, round)
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

                // Wait until we receive the block hash, even if we have no use for it yet.
                let _block_hash = rx_hash.await?;

                let block_parts = state.part_store.all_parts(height, round);
                if let Some((value, _)) = self.build_proposal_content(&block_parts, height, round) {
                    let proposed_value = LocallyProposedValue::new(height, round, value);
                    reply_to.send(proposed_value)?;
                }

                Ok(())
            }

            HostMsg::ReceivedBlockPart {
                block_part,
                reply_to,
            } => {
                let value = self.build_value_from_block_part(state, block_part).await;
                reply_to.send(value)?;

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

            HostMsg::DecidedOnValue { .. } => {
                todo!()
            }
        }
    }
}
