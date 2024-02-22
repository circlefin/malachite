use std::marker::PhantomData;
use std::time::Instant;

use derive_where::derive_where;

use futures::future::BoxFuture;
use malachite_common::Context;

#[allow(async_fn_in_trait)]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    fn build_proposal(
        &self,
        height: Ctx::Height,
        deadline: Instant,
    ) -> BoxFuture<Option<Ctx::Value>>;
}

pub mod test {
    use super::*;

    use futures::FutureExt;
    use malachite_test::{Height, TestContext, Value};

    #[derive_where(Default)]
    pub struct TestValueBuilder<Ctx: Context> {
        _phantom: PhantomData<Ctx>,
    }

    impl ValueBuilder<TestContext> for TestValueBuilder<TestContext> {
        fn build_proposal(&self, height: Height, deadline: Instant) -> BoxFuture<Option<Value>> {
            async move {
                let diff = deadline.duration_since(Instant::now());
                let wait = diff / 2;

                tokio::time::sleep(wait).await;

                Some(Value::new(40 + height.as_u64()))
            }
            .boxed()
        }
    }
}
