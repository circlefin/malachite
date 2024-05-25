use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;

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
            _timeout_duration: Duration,
            validator_address: Address,
        ) -> Option<Value> {
            // TODO - loop, execute, stop on timeout and send blockID
            let txes = self
                .tx_streamer
                .call(
                    |reply| crate::mempool::Msg::TxStream {
                        height: height.as_u64(),
                        reply,
                    },
                    None,
                ) // TODO timeout
                .await
                .ok()?
                .unwrap();

            let block_part = BlockPart {
                height,
                round,
                sequence: 1,
                transactions: txes.clone(),
                validator_address,
            };

            self.batch_gossip
                .as_ref()
                .unwrap()
                .cast(crate::consensus::Msg::BuilderBlockPart(block_part))
                .unwrap(); // FIXME

            tokio::time::sleep(Duration::from_millis(10)).await;
            Some(Value::new(txes))
        }
    }
}
