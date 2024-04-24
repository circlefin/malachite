use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;
use derive_where::derive_where;

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

    #[derive_where(Default)]
    pub struct TestValueBuilder<Ctx: Context> {
        _phantom: PhantomData<Ctx>,
    }

    #[async_trait]
    impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
        async fn build_value(&self, height: Height, timeout_duration: Duration) -> Option<Value> {
            tokio::time::sleep(timeout_duration / 2).await;

            Some(Value::new(40 + height.as_u64()))
        }
    }
}
