use async_trait::async_trait;

use malachite_common::Round;
use malachite_driver::Env;

use crate::{Height, TestContext, Value};

pub struct TestEnv {
    get_value: Box<dyn Fn(Height, Round) -> Option<Value> + Send + Sync>,
}

impl TestEnv {
    pub fn new(get_value: impl Fn(Height, Round) -> Option<Value> + Send + Sync + 'static) -> Self {
        Self {
            get_value: Box::new(get_value),
        }
    }
}

#[async_trait]
impl Env<TestContext> for TestEnv {
    async fn get_value(&self, height: Height, round: Round) -> Option<Value> {
        (self.get_value)(height, round)
    }
}
