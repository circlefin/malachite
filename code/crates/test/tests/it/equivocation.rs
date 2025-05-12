use informalsystems_malachitebft_test::{
    middleware::Middleware, Address, Height, TestContext, Value, ValueId, Vote,
};
use malachitebft_core_consensus::LocallyProposedValue;
use malachitebft_core_types::{NilOrVal, Round};
use rand::Rng;

#[derive(Copy, Clone, Debug)]
struct _EquivocationProposer;

impl Middleware for _EquivocationProposer {
    fn on_propose_value(
        &self,
        _ctx: &TestContext,
        proposal: &mut LocallyProposedValue<informalsystems_malachitebft_test::TestContext>,
        reproposal: bool,
    ) {
        if !reproposal {
            tracing::warn!(
                "EquivocationProposer: First time proposing value {:}",
                proposal.value.id()
            );

            // Keep the proposal value as is
            return;
        }

        // Change the proposal value to a different one
        let new_value = loop {
            let new_value = Value::new(rand::thread_rng().gen_range(100..=100000));
            if new_value != proposal.value {
                break new_value;
            }
        };

        tracing::warn!(
            "EquivocationProposer: Reproposing value {:} instead of {:}",
            new_value.id(),
            proposal.value.id()
        );

        proposal.value = new_value;
    }
}

#[derive(Clone, Debug)]
struct _EquivocationVoter;

impl Middleware for _EquivocationVoter {
    fn new_prevote(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        if round.as_i64() % 2 == 0 {
            // Vote for the given value
            Vote::new_prevote(height, round, value_id, address)
        } else {
            // Vote for a different value
            let new_value = loop {
                let new_value = ValueId::new(rand::thread_rng().gen_range(100..=100000));
                if NilOrVal::Val(new_value) != value_id {
                    break new_value;
                }
            };

            Vote::new_prevote(height, round, NilOrVal::Val(new_value), address)
        }
    }
}
