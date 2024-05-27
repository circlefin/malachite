use async_trait::async_trait;
use std::marker::PhantomData;
use std::time::{Duration, Instant};
use tracing::info;

use malachite_common::{Context, Round};

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_value(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
    ) -> Option<Ctx::Value>;
}

pub mod test {
    // TODO - parameterize
    const NUM_TXES_PER_PART: u64 = 2;
    const TIME_ALLOWANCE_FACTOR: f32 = 0.75;
    const EXEC_TIME_MICROSEC_PER_PART: u64 = 500;
    use super::*;

    use malachite_test::{Address, BlockPart, Height, TestContext, Value};
    use ractor::ActorRef;

    #[derive(Clone)]
    pub struct TestValueBuilder<Ctx: Context> {
        _phantom: PhantomData<Ctx>,
        tx_streamer: ActorRef<crate::mempool::Msg>,
        pub batch_gossip: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
    }

    impl<Ctx> TestValueBuilder<Ctx>
    where
        Ctx: Context,
    {
        pub fn new(
            tx_streamer: ActorRef<crate::mempool::Msg>,
            batch_gossip: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
        ) -> Self {
            Self {
                _phantom: Default::default(),
                tx_streamer,
                batch_gossip,
            }
        }
    }

    #[async_trait]
    impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
        async fn build_value(
            &self,
            height: Height,
            round: Round,
            timeout_duration: Duration,
            validator_address: Address,
        ) -> Option<Value> {
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
                // Simulate execution
                tokio::time::sleep(Duration::from_micros(EXEC_TIME_MICROSEC_PER_PART)).await;

                // Send the batch in a BlockPart
                let block_part = BlockPart {
                    height,
                    round,
                    sequence,
                    transactions: txes.clone(),
                    validator_address,
                };

                if self.batch_gossip.as_ref().is_some() {
                    // TODO - this will never be reached due to init problems with batch_gossip
                    // Once fixed remove the if and the Option from batch_gossip.
                    self.batch_gossip
                        .as_ref()
                        .unwrap()
                        .cast(crate::consensus::Msg::BuilderBlockPart(block_part.clone()))
                        .unwrap(); // FIXME
                }

                tx_batch.append(&mut txes);

                if Instant::now().gt(&finish_time) {
                    break;
                }
                sequence += 1;
            }
            info!(
                "Value Builder created a block with {} tx-es",
                tx_batch.len()
            );
            Some(Value::new(tx_batch))
        }
    }
}
