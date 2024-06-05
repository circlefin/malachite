use std::time::{Duration, Instant};

use async_trait::async_trait;
use tracing::{error, info, trace};

use malachite_common::{Context, Round};

use crate::consensus::{ConsensusRef, Msg as ConsensusMsg};
use crate::host::{LocallyProposedValue, ReceivedProposedValue};
use crate::util::PartStore;

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_value_locally(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        consensus: ConsensusRef<Ctx>,
        part_store: &mut PartStore<Ctx>,
    ) -> Option<LocallyProposedValue<Ctx>>;

    async fn build_value_from_block_parts(
        &self,
        block_part: Ctx::BlockPart,
        part_store: &mut PartStore<Ctx>,
    ) -> Option<ReceivedProposedValue<Ctx>>;

    async fn maybe_received_value(
        &self,
        height: Ctx::Height,
        round: Round,
        part_store: &mut PartStore<Ctx>,
    ) -> Option<ReceivedProposedValue<Ctx>>;
}

pub mod test {
    use super::*;

    use std::marker::PhantomData;

    use bytesize::ByteSize;

    use malachite_common::Context;
    use malachite_driver::Validity;
    use malachite_test::{
        Address, BlockMetadata, BlockPart, Content, Height, TestContext, TransactionBatch, Value,
    };

    use crate::mempool::{MempoolRef, Msg as MempoolMsg};

    #[derive(Copy, Clone, Debug)]
    pub struct TestParams {
        pub max_block_size: ByteSize,
        pub tx_size: ByteSize,
        pub txs_per_part: u64,
        pub time_allowance_factor: f32,
        pub exec_time_per_part: Duration,
    }

    #[derive(Clone)]
    pub struct TestValueBuilder<Ctx: Context> {
        tx_streamer: MempoolRef,
        params: TestParams,
        _phantom: PhantomData<Ctx>,
    }

    impl<Ctx> TestValueBuilder<Ctx>
    where
        Ctx: Context,
    {
        pub fn new(tx_streamer: MempoolRef, params: TestParams) -> Self {
            Self {
                tx_streamer,
                params,
                _phantom: PhantomData,
            }
        }
    }

    #[async_trait]
    impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
        async fn build_value_locally(
            &self,
            height: Height,
            round: Round,
            timeout_duration: Duration,
            validator_address: Address,
            consensus: ConsensusRef<TestContext>,
            part_store: &mut PartStore<TestContext>,
        ) -> Option<LocallyProposedValue<TestContext>> {
            let now = Instant::now();
            let deadline = now + timeout_duration.mul_f32(self.params.time_allowance_factor);
            let expiration_time = now + timeout_duration;

            let mut tx_batch = vec![];
            let mut sequence = 1;
            let mut block_size = 0;
            let mut result = None;

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
                            tx_size: self.params.tx_size,
                            num_txes: self.params.txs_per_part,
                            reply,
                        },
                        None,
                    ) // TODO timeout
                    .await
                    .ok()?
                    .unwrap();

                if txes.is_empty() {
                    break;
                }

                // Create, store and gossip the batch in a BlockPart
                let block_part = BlockPart::new(
                    height,
                    round,
                    sequence,
                    validator_address,
                    Content::new(TransactionBatch::new(txes.clone()), None),
                );

                part_store.store(block_part.clone());

                consensus
                    .cast(ConsensusMsg::BuilderBlockPart(block_part.clone()))
                    .unwrap();

                // Simulate execution
                tokio::time::sleep(self.params.exec_time_per_part).await;

                'inner: for tx in txes {
                    if block_size + tx.size_bytes() > self.params.max_block_size.as_u64() {
                        break 'inner;
                    }

                    block_size += tx.size_bytes();
                    tx_batch.push(tx);
                }

                sequence += 1;

                if Instant::now() > expiration_time {
                    error!("Value Builder started at {now:?} but failed to complete by expiration time {expiration_time:?}");
                    result = None;
                    break;
                }

                if Instant::now() > deadline {
                    // Create, store and gossip the BlockMetadata in a BlockPart
                    let value = Value::new_from_transactions(tx_batch.clone());

                    result = Some(LocallyProposedValue {
                        height,
                        round,
                        value: Some(value),
                    });

                    let block_part = BlockPart::new(
                        height,
                        round,
                        sequence,
                        validator_address,
                        Content::new(
                            TransactionBatch::new(vec![]),
                            Some(BlockMetadata::new(vec![], value)),
                        ),
                    );

                    part_store.store(block_part.clone());

                    consensus
                        .cast(ConsensusMsg::BuilderBlockPart(block_part))
                        .unwrap();

                    info!(
                        "Value Builder created a block with {} tx-es ({}), block hash: {:?} ",
                        tx_batch.len(),
                        ByteSize::b(block_size),
                        value.id()
                    );

                    break;
                }
            }

            result
        }

        async fn build_value_from_block_parts(
            &self,
            block_part: BlockPart,
            part_store: &mut PartStore<TestContext>,
        ) -> Option<ReceivedProposedValue<TestContext>> {
            let height = block_part.height();
            let round = block_part.round();
            let sequence = block_part.sequence();

            part_store.store(block_part.clone());
            let num_parts = part_store.all_parts(height, round).len();
            trace!("({num_parts}): Received block part (h: {height}, r: {round}, seq: {sequence}");

            // Simulate Tx execution and proof verification (assumes success)
            // TODO - add config knob for invalid blocks
            tokio::time::sleep(self.params.exec_time_per_part).await;

            // Get the "last" part, the one with highest sequence.
            // Block parts may not be received in order.
            if let Some(last_part) =
                part_store.get(block_part.height(), block_part.round(), num_parts as u64)
            {
                // If the "last" part includes a metadata then this is truly the last part.
                // So in this case all block parts have been received, including the metadata that includes
                // the block hash/ value. This can be returned as the block is complete.
                // TODO - the logic here is weak, we assume earlier parts don't include metadata
                // Should change once we implement `oneof`/ proper enum in protobuf but good enough for now test code
                match last_part.metadata() {
                    Some(meta) => {
                        info!(
                            "Value Builder received last block part for height:{}, round:{}, num_parts: {num_parts}",
                            last_part.height(),
                            last_part.round(),
                        );
                        Some(ReceivedProposedValue {
                            validator_address: *last_part.validator_address(),
                            height: last_part.height(),
                            round: last_part.round(),
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
            &self,
            height: Height,
            round: Round,
            part_store: &mut PartStore<TestContext>,
        ) -> Option<ReceivedProposedValue<TestContext>> {
            let block_parts = part_store.all_parts(height, round);
            let num_parts = block_parts.len();
            let last_part = block_parts[num_parts - 1];
            last_part.metadata().map(|metadata| ReceivedProposedValue {
                validator_address: *last_part.validator_address(),
                height,
                round,
                value: Some(metadata.value()),
                valid: Validity::Valid,
            })
        }
    }
}
