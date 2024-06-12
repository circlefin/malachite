use std::hash::{DefaultHasher, Hash, Hasher};
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use bytesize::ByteSize;
use tracing::{debug, error, info, trace};

use malachite_actors::consensus::Metrics;
use malachite_actors::consensus::{ConsensusRef, Msg as ConsensusMsg};
use malachite_actors::host::{LocallyProposedValue, ReceivedProposedValue};
use malachite_actors::mempool::{MempoolRef, Msg as MempoolMsg};
use malachite_actors::value_builder::ValueBuilder;
use malachite_common::{Context, Round, TransactionBatch};
use malachite_driver::Validity;
use malachite_test::{Address, BlockMetadata, BlockPart, Content, Height, TestContext, Value};

use crate::part_store::PartStore;

#[derive(Copy, Clone, Debug)]
pub struct TestParams {
    pub max_block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
}

pub struct TestValueBuilder<Ctx: Context> {
    tx_streamer: MempoolRef,
    params: TestParams,
    part_store: PartStore<TestContext>,
    metrics: Metrics,
    _phantom: PhantomData<Ctx>,
}

impl<Ctx> TestValueBuilder<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        tx_streamer: MempoolRef,
        params: TestParams,
        part_store: PartStore<TestContext>,
        metrics: Metrics,
    ) -> Self {
        Self {
            tx_streamer,
            params,
            metrics,
            part_store,
            _phantom: PhantomData,
        }
    }
}

#[async_trait]
impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
    #[tracing::instrument(
            name = "value_builder.locally",
            skip_all,
            fields(
                height = %height,
                round = %round,
            )
        )]
    async fn build_value_locally(
        &mut self,
        height: Height,
        round: Round,
        timeout_duration: Duration,
        validator_address: Address,
        consensus: ConsensusRef<TestContext>,
    ) -> Option<LocallyProposedValue<TestContext>> {
        let start = Instant::now();
        let deadline = start + timeout_duration.mul_f32(self.params.time_allowance_factor);
        let expiration_time = start + timeout_duration;

        let mut tx_batch = vec![];
        let mut sequence = 1;
        let mut block_size = 0;

        loop {
            trace!(
                "Build local value for h:{}, r:{}, s:{}",
                height,
                round,
                sequence
            );

            let txes = self
                .tx_streamer
                .call(
                    |reply| MempoolMsg::TxStream {
                        height: height.as_u64(),
                        num_txes: self.params.txs_per_part,
                        reply,
                    },
                    None,
                ) // TODO timeout
                .await
                .ok()?
                .unwrap();

            if txes.is_empty() {
                return None;
            }

            // Create, store and gossip the batch in a BlockPart
            let block_part = BlockPart::new(
                height,
                round,
                sequence,
                validator_address,
                Content::TxBatch(TransactionBatch::new(txes.clone())),
            );

            self.part_store.store(block_part.clone());

            consensus
                .cast(ConsensusMsg::BuilderBlockPart(block_part))
                .unwrap();

            let mut tx_count = 0;

            'inner: for tx in txes {
                if block_size + tx.size_bytes() > self.params.max_block_size.as_u64() as usize {
                    break 'inner;
                }

                block_size += tx.size_bytes();
                tx_batch.push(tx);
                tx_count += 1;
            }

            // Simulate execution of reaped txes
            let exec_time = self.params.exec_time_per_tx * tx_count;
            debug!("Simulating tx execution for {tx_count} tx-es, sleeping for {exec_time:?}");
            tokio::time::sleep(exec_time).await;

            if Instant::now() > expiration_time {
                error!(
                    "Value Builder failed to complete in given interval ({timeout_duration:?}), took {:?}",
                    Instant::now() - start,
                );

                return None;
            }

            sequence += 1;

            if Instant::now() > deadline {
                // Create, store and gossip the BlockMetadata in a BlockPart
                let value = Value::new_from_transactions(&tx_batch);

                let result = Some(LocallyProposedValue {
                    height,
                    round,
                    value: Some(value),
                });

                let block_part = BlockPart::new(
                    height,
                    round,
                    sequence,
                    validator_address,
                    Content::Metadata(BlockMetadata::new(vec![], value)),
                );

                self.part_store.store(block_part.clone());

                consensus
                    .cast(ConsensusMsg::BuilderBlockPart(block_part))
                    .unwrap();

                info!(
                    "Value Builder created a block with {} tx-es of size {} in {:?} with hash {:?} ",
                    tx_batch.len(),
                    ByteSize::b(block_size as u64),
                    Instant::now() - start,
                    value.id()
                );

                return result;
            }
        }
    }

    #[tracing::instrument(
            name = "value_builder.from_block_parts",
            skip_all,
            fields(
                height = %block_part.height,
                round = %block_part.round,
                sequence = %block_part.sequence
            )
        )]
    async fn build_value_from_block_parts(
        &mut self,
        block_part: BlockPart,
    ) -> Option<ReceivedProposedValue<TestContext>> {
        let height = block_part.height;
        let round = block_part.round;
        let sequence = block_part.sequence;

        self.part_store.store(block_part.clone());
        let all_parts = self.part_store.all_parts(height, round);

        trace!(%height, %round, %sequence, "Received block part");

        // Simulate Tx execution and proof verification (assumes success)
        // TODO - add config knob for invalid blocks
        let num_txes = block_part.content.tx_count().unwrap_or(0) as u32;
        tokio::time::sleep(self.params.exec_time_per_tx * num_txes).await;

        // Get the "last" part, the one with highest sequence.
        // Block parts may not be received in order.
        let highest_sequence = all_parts.len() as u64;

        if let Some(last_part) = self.part_store.get(height, round, highest_sequence) {
            // If the "last" part includes a metadata then this is truly the last part.
            // So in this case all block parts have been received, including the metadata that includes
            // the block hash/ value. This can be returned as the block is complete.
            // TODO - the logic here is weak, we assume earlier parts don't include metadata
            // Should change once we implement `oneof`/ proper enum in protobuf but good enough for now test code
            match last_part.metadata() {
                Some(meta) => {
                    let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
                    let tx_count: usize = all_parts
                        .iter()
                        .map(|p| p.content.tx_count().unwrap_or(0))
                        .sum();

                    info!(
                        height = %last_part.height,
                        round = %last_part.round,
                        tx_count = %tx_count,
                        block_size = %block_size,
                        num_parts = %all_parts.len(),
                        "Value Builder received last block part",
                    );

                    Some(ReceivedProposedValue {
                        validator_address: last_part.validator_address,
                        height: last_part.height,
                        round: last_part.round,
                        value: Some(meta.value()),
                        valid: Validity::Valid,
                    })
                }
                None => None,
            }
        } else {
            None
        }
    }

    async fn maybe_received_value(
        &mut self,
        height: Height,
        round: Round,
    ) -> Option<ReceivedProposedValue<TestContext>> {
        let block_parts = self.part_store.all_parts(height, round);
        let num_parts = block_parts.len();
        let last_part = block_parts[num_parts - 1];

        last_part.metadata().map(|metadata| ReceivedProposedValue {
            validator_address: last_part.validator_address,
            height,
            round,
            value: Some(metadata.value()),
            valid: Validity::Valid,
        })
    }

    #[tracing::instrument(
            name = "value_builder.decided",
            skip_all,
            fields(
            height = %height,
            round = %round,
            )
        )]
    async fn decided_on_value(&mut self, height: Height, round: Round, value: Value) {
        info!("Build and store block with hash {value:?}");

        let all_parts = self.part_store.all_parts(height, round);

        // TODO - build the block from block parts and store it

        // Update metrics
        let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
        let tx_count: usize = all_parts
            .iter()
            .map(|p| p.content.tx_count().unwrap_or(0))
            .sum();

        self.metrics.block_tx_count.observe(tx_count as f64);
        self.metrics.block_size_bytes.observe(block_size as f64);
        self.metrics.finalized_txes.inc_by(tx_count as u64);

        // Send Update to mempool to remove all the tx-es included in the block.
        let mut tx_hashes = vec![];
        for part in all_parts {
            if let Content::TxBatch(transaction_batch) = part.content.as_ref() {
                tx_hashes.extend(transaction_batch.transactions().iter().map(|tx| {
                    let mut hash = DefaultHasher::new();
                    tx.0.hash(&mut hash);
                    hash.finish()
                }));
            }
        }
        let _ = self.tx_streamer.cast(MempoolMsg::Update { tx_hashes });

        // Prune the PartStore of all parts for heights lower than `height - 1`
        self.part_store.prune(height.decrement().unwrap_or(height));
    }
}
