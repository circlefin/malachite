use malachite_common::{Context, Proposal, Round, Value, VoteType};
use malachite_round::input::Input as RoundInput;
use malachite_round::state::Step;
use malachite_vote::keeper::VoteKeeper;
use malachite_vote::Threshold;

pub fn multiplex_proposal<Ctx>(
    input: RoundInput<Ctx>,
    input_round: Round,
    proposal: Option<&Ctx::Proposal>,
) -> RoundInput<Ctx>
where
    Ctx: Context,
{
    match input {
        // Check if we have a proposal for the input round,
        // if so, send `ProposalAndPolkaCurrent` instead of `PolkaAny`
        // to the state machine.
        RoundInput::PolkaValue(value_id) => {
            if let Some(proposal) = proposal {
                assert_eq!(proposal.round(), input_round);
                assert_eq!(proposal.value().id(), value_id);

                RoundInput::ProposalAndPolkaCurrent(proposal.clone())
            } else {
                RoundInput::PolkaAny
            }
        }

        // Check if we have a proposal for the input round,
        // if so, send `ProposalAndPrecommitValue` instead of `PrecommitAny`.
        RoundInput::PrecommitValue(value_id) => {
            if let Some(proposal) = proposal {
                assert_eq!(proposal.round(), input_round);
                assert_eq!(proposal.value().id(), value_id);

                RoundInput::ProposalAndPrecommitValue(proposal.clone())
            } else {
                RoundInput::PrecommitAny
            }
        }

        // Otherwise, just pass the input through.
        _ => input,
    }
}
pub fn multiplex_on_step_change<Ctx>(
    pending_step: Step,
    round: Round,
    votekeeper: &VoteKeeper<Ctx>,
    proposal: Option<&Ctx::Proposal>,
) -> Option<RoundInput<Ctx>>
where
    Ctx: Context,
{
    match pending_step {
        Step::NewRound => None, // Some(RoundInput::NewRound),

        Step::Prevote => {
            if has_polka_nil(votekeeper, round) {
                Some(RoundInput::PolkaNil)
            } else if let Some(proposal) = has_polka_value(proposal, votekeeper, round) {
                Some(RoundInput::ProposalAndPolkaCurrent(proposal.clone()))
            } else if has_polka_any(votekeeper, round) {
                Some(RoundInput::PolkaAny)
            } else {
                None
            }
        }

        Step::Propose => None,
        Step::Precommit => None,
        Step::Commit => None,
    }
}

fn has_polka_nil<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Nil)
}

fn has_polka_value<'p, Ctx>(
    proposal: Option<&'p Ctx::Proposal>,
    votekeeper: &VoteKeeper<Ctx>,
    round: Round,
) -> Option<&'p Ctx::Proposal>
where
    Ctx: Context,
{
    let proposal = proposal?;

    votekeeper
        .is_threshold_met(
            &round,
            VoteType::Prevote,
            Threshold::Value(proposal.value().id()),
        )
        .then_some(proposal)
}

fn has_polka_any<Ctx>(votekeeper: &VoteKeeper<Ctx>, round: Round) -> bool
where
    Ctx: Context,
{
    votekeeper.is_threshold_met(&round, VoteType::Prevote, Threshold::Any)
}
