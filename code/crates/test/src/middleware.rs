use core::fmt;

use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue};
use malachitebft_core_types::{CommitCertificate, NilOrVal, Round};

use crate::{Address, Height, Proposal, TestContext, Value, ValueId, Vote};

pub trait Middleware: fmt::Debug + Send + Sync {
    fn new_proposal(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        Proposal::new(height, round, value, pol_round, address)
    }

    fn new_prevote(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        &self,
        _ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }

    fn on_propose_value(
        &self,
        _ctx: &TestContext,
        _proposed_value: &mut LocallyProposedValue<TestContext>,
        _reproposal: bool,
    ) {
    }

    fn on_commit(
        &self,
        _ctx: &TestContext,
        _certificate: &CommitCertificate<TestContext>,
        _proposal: &ProposedValue<TestContext>,
    ) -> Result<(), eyre::Report> {
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct DefaultMiddleware;

impl Middleware for DefaultMiddleware {}
