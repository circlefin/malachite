use async_trait::async_trait;

use malachite_driver::Env;

use crate::{Proposal, TestContext, Value};

pub struct TestEnv {
    pub value: Value,
    pub is_valid: fn(&Proposal) -> bool,
}

impl TestEnv {
    pub fn new(value: Value, is_valid: fn(&Proposal) -> bool) -> Self {
        Self { value, is_valid }
    }
}

#[async_trait]
impl Env<TestContext> for TestEnv {
    async fn get_value(&self) -> Value {
        self.value.clone()
    }

    async fn validate_proposal(&self, proposal: &Proposal) -> bool {
        (self.is_valid)(proposal)
    }
}
