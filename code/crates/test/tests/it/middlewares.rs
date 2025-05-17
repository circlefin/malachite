use informalsystems_malachitebft_test::{self as malachitebft_test};

use malachitebft_core_consensus::LocallyProposedValue;
use malachitebft_core_types::{NilOrVal, Round};
use malachitebft_test::middleware::Middleware;
use malachitebft_test::{Address, Height, TestContext, ValueId, Vote};

#[derive(Copy, Clone, Debug)]
pub struct ByzantineProposer;

impl Middleware for ByzantineProposer {
    fn on_propose_value(
        &self,
        _ctx: &TestContext,
        proposal: &mut LocallyProposedValue<TestContext>,
        reproposal: bool,
    ) {
        use informalsystems_malachitebft_test::Value;
        use rand::Rng;

        if !reproposal {
            tracing::warn!(
                "ByzantineProposer: First time proposing value {:}",
                proposal.value.id()
            );

            // Do not change the value if it is the first time we propose it
            return;
        }

        // Make up a new value that is different from the one we are supposed to propose
        let new_value = loop {
            let new_value = Value::new(rand::thread_rng().gen_range(100..=100000));
            if new_value != proposal.value {
                break new_value;
            }
        };

        tracing::warn!(
            "ByzantineProposer: Not re-using previously built value {:} but a new one {:}",
            proposal.value.id(),
            new_value.id()
        );

        proposal.value = new_value;
    }
}

#[derive(Copy, Clone, Debug)]
pub struct PrevoteNil {
    enabled: fn(Height, Round, &NilOrVal<ValueId>) -> bool,
}

impl PrevoteNil {
    pub fn when(enabled: fn(Height, Round, &NilOrVal<ValueId>) -> bool) -> Self {
        Self { enabled }
    }
}

impl Middleware for PrevoteNil {
    fn new_prevote(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        if (self.enabled)(height, round, &value_id) {
            Vote::new_prevote(height, round, NilOrVal::Nil, address)
        } else {
            Vote::new_prevote(height, round, value_id, address)
        }
    }
}
