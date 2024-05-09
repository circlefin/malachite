use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;

use malachite_common::Context;

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_value(
        &self,
        height: Ctx::Height,
        timeout_duration: Duration,
    ) -> Option<Ctx::Value>;
}

pub mod test {
    use super::*;

    use malachite_test::{Height, TestContext, Value};
    use ractor::ActorRef;

    pub struct TestValueBuilder<Ctx: Context> {
        _phantom: PhantomData<Ctx>,
        tx_streamer: ActorRef<crate::mempool::Msg>,
    }

    impl<Ctx> TestValueBuilder<Ctx>
    where
        Ctx: Context,
    {
        pub fn new(tx_streamer: ActorRef<crate::mempool::Msg>) -> Self {
            Self {
                _phantom: Default::default(),
                tx_streamer,
            }
        }
    }

    #[async_trait]
    impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
        async fn build_value(&self, height: Height, _timeout_duration: Duration) -> Option<Value> {
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

            tokio::time::sleep(Duration::from_millis(10)).await;
            Some(Value(txes))
        }
    }
}
