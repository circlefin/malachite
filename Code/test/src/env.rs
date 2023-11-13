use async_trait::async_trait;

use malachite_common::Round;
use malachite_driver::Env;

use crate::{Height, Proposal, TestContext, Value};

pub struct TestEnv {
    get_value: Box<dyn Fn(Height, Round) -> Option<Value> + Send + Sync>,
    is_valid: Box<dyn Fn(&Proposal) -> bool + Send + Sync>,
}

impl TestEnv {
    pub fn new(
        get_value: impl Fn(Height, Round) -> Option<Value> + Send + Sync + 'static,
        is_valid: impl Fn(&Proposal) -> bool + Send + Sync + 'static,
    ) -> Self {
        Self {
            get_value: Box::new(get_value),
            is_valid: Box::new(is_valid),
        }
    }
}

#[async_trait]
impl Env<TestContext> for TestEnv {
    async fn get_value(&self, height: Height, round: Round) -> Option<Value> {
        (self.get_value)(height, round)
    }

    async fn validate_proposal(&self, proposal: &Proposal) -> bool {
        (self.is_valid)(proposal)
    }
}
