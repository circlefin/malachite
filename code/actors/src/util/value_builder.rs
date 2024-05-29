use async_trait::async_trait;
use ractor::ActorRef;
use std::marker::PhantomData;
use std::time::{Duration, Instant};
use tracing::info;

use malachite_common::{Context, Round};

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_value_locally(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        gossip_actor: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
    ) -> Option<Ctx::Value>;

    async fn build_value_from_block_parts(&self, block_part: Ctx::BlockPart) -> Option<Ctx::Value>;
}

pub mod test {
    // TODO - parameterize
    // If based on the propose_timeout and the constants below we end up with more than 300 parts then consensus
    // is never reached in a round and we keep moving to the next one.
    const NUM_TXES_PER_PART: u64 = 300;
    const TIME_ALLOWANCE_FACTOR: f32 = 0.5;
    const EXEC_TIME_MICROSEC_PER_PART: u64 = 100000;

    use super::*;
    use std::collections::BTreeMap;

    use malachite_test::{
        Address, BlockMetadata, BlockPart, Content, Height, TestContext, TransactionBatch, Value,
    };
    use ractor::ActorRef;

    #[derive(Clone)]
    pub struct TestValueBuilder<Ctx: Context> {
        _phantom: PhantomData<Ctx>,
        tx_streamer: ActorRef<crate::mempool::Msg>,
        part_map: BTreeMap<(Height, Round, u64), BlockPart>,
    }

    impl<Ctx> TestValueBuilder<Ctx>
    where
        Ctx: Context,
    {
        pub fn new(tx_streamer: ActorRef<crate::mempool::Msg>) -> Self {
            Self {
                _phantom: Default::default(),
                tx_streamer,
                part_map: BTreeMap::new(),
            }
        }
        pub fn get(&self, height: Height, round: Round, sequence: u64) -> Option<&BlockPart> {
            self.part_map.get(&(height, round, sequence))
        }
        pub fn store(&mut self, block_part: BlockPart) {
            let height = block_part.height();
            let round = block_part.round();
            let sequence = block_part.sequence();
            self.part_map
                .entry((height, round, sequence))
                .or_insert(block_part);
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
            gossip_actor: Option<ActorRef<crate::consensus::Msg<TestContext>>>,
        ) -> Option<Value> {
            let mut result = None;
            let finish_time = Instant::now() + timeout_duration.mul_f32(TIME_ALLOWANCE_FACTOR);

            let mut tx_batch = vec![];
            let mut sequence = 1;
            loop {
                let mut txes = self
                    .tx_streamer
                    .call(
                        |reply| crate::mempool::Msg::TxStream {
                            height: height.as_u64(),
                            num_txes: NUM_TXES_PER_PART,
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

                // TODO:
                // ^^^^ `__self` is a `&` reference, so the data it refers to cannot be borrowed as mutable
                //self.store(block_part.clone());

                gossip_actor
                    .as_ref()
                    .unwrap()
                    .cast(crate::consensus::Msg::<TestContext>::BuilderBlockPart(
                        block_part.clone(),
                    ))
                    .unwrap();

                // Simulate execution
                tokio::time::sleep(Duration::from_micros(EXEC_TIME_MICROSEC_PER_PART)).await;
                tx_batch.append(&mut txes);

                sequence += 1;

                if Instant::now().gt(&finish_time) {
                    // Create, store and gossip the BlockMetadata in a BlockPart
                    let value = Value::new_from_transactions(tx_batch.clone());
                    result = Some(value);
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

                    //self.store(block_part.clone());

                    gossip_actor
                        .as_ref()
                        .unwrap()
                        .cast(crate::consensus::Msg::<TestContext>::BuilderBlockPart(
                            block_part.clone(),
                        ))
                        .unwrap();

                    break;
                }
            }
            info!(
                "Value Builder created a block with {} tx-es, block hash (consensus value) {:?} ",
                tx_batch.len(),
                result.clone()
            );

            result
        }

        async fn build_value_from_block_parts(
            &self,
            block_part: BlockPart,
        ) -> Option<<TestContext as malachite_common::Context>::Value> {
            if block_part.sequence() % 10 == 0 {
                info!(
                    "Received block part (h: {}, r: {}, seq: {}",
                    block_part.height(),
                    block_part.round(),
                    block_part.sequence()
                );
            }
            // TODO - implement block part logic
            // - store the part:
            //     self.store(block_part);
            // - determine if all parts have been received
            //   - the BlockMetadata sequence is the total number of parts
            // - reduce attack vector
            //   - these have been signed by sender and verified by consensus before being forwarded here
            //   - still there should be limits put in place
            // - should support multiple proposals in parallel
            // - should fix the APIs to confirm with the "Context APIs" from Starkware
            None
        }
    }
}
