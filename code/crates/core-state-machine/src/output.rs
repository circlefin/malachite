//! Outputs of the round state machine.

use derive_where::derive_where;

use malachite_core_types::{Context, NilOrVal, Round, Timeout, TimeoutKind, ValueId};

/// Output of the round state machine.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Output<Ctx>
where
    Ctx: Context,
{
    /// Move to the new round.
    NewRound(Round),

    /// Broadcast the proposal.
    Proposal(Ctx::Proposal),

    /// Broadcast the vote.
    Vote(Ctx::Vote),

    /// Schedule the timeout.
    ScheduleTimeout(Timeout),

    /// Ask for a value at the given height, round and to schedule a timeout.
    /// The timeout tells the proposal builder how long it has to build a value.
    GetValueAndScheduleTimeout(Ctx::Height, Round, Timeout),

    /// Decide the value.
    Decision(Round, Ctx::Proposal),
}

impl<Ctx: Context> Output<Ctx> {
    /// Build a `Proposal` output.
    pub fn proposal(
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        pol_round: Round,
        address: Ctx::Address,
    ) -> Self {
        Output::Proposal(Ctx::new_proposal(height, round, value, pol_round, address))
    }

    /// Build a `Vote` output for a prevote.
    pub fn prevote(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_prevote(height, round, value_id, address))
    }

    /// Build a `Vote` output for a precommit.
    pub fn precommit(
        height: Ctx::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Ctx>>,
        address: Ctx::Address,
    ) -> Self {
        Output::Vote(Ctx::new_precommit(height, round, value_id, address))
    }

    /// Build a `ScheduleTimeout` output.
    pub fn schedule_timeout(round: Round, step: TimeoutKind) -> Self {
        Output::ScheduleTimeout(Timeout { round, kind: step })
    }

    /// Build a `GetValue` output.
    pub fn get_value_and_schedule_timeout(
        height: Ctx::Height,
        round: Round,
        step: TimeoutKind,
    ) -> Self {
        Output::GetValueAndScheduleTimeout(height, round, Timeout { round, kind: step })
    }

    /// Build a `Decision` output.
    pub fn decision(round: Round, proposal: Ctx::Proposal) -> Self {
        Output::Decision(round, proposal)
    }
}
