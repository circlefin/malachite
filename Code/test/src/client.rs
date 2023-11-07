use malachite_consensus::executor::Client;

use crate::{Proposal, TestContext, Value};

pub struct TestClient {
    pub value: Value,
    pub is_valid: fn(&Proposal) -> bool,
}

impl TestClient {
    pub fn new(value: Value, is_valid: fn(&Proposal) -> bool) -> Self {
        Self { value, is_valid }
    }
}

impl Client<TestContext> for TestClient {
    fn get_value(&self) -> Value {
        self.value.clone()
    }

    fn validate_proposal(&self, proposal: &Proposal) -> bool {
        (self.is_valid)(proposal)
    }
}
