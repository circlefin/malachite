use std::marker::PhantomData;
use std::time::Instant;

use async_trait::async_trait;
use derive_where::derive_where;

use malachite_common::Context;

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_proposal(&self, height: Ctx::Height, deadline: Instant) -> Option<Ctx::Value>;
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
        async fn build_proposal(&self, height: Height, deadline: Instant) -> Option<Value> {
            let diff = deadline.duration_since(Instant::now());
            let wait = diff / 2;

            tokio::time::sleep(wait).await;

            Some(Value::new(40 + height.as_u64()))
        }
    }
}
