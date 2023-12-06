use malachite_common::{Context, Proposal, Round, Value};
use malachite_round::input::Input as RoundInput;

use crate::proposals::Proposals;

pub fn multiplex_event<Ctx>(
    input: RoundInput<Ctx>,
    input_round: Round,
    proposals: &Proposals<Ctx>,
) -> RoundInput<Ctx>
where
    Ctx: Context,
{
    match input {
        RoundInput::PolkaValue(value_id) => {
            let proposal = proposals.find(&value_id, |p| p.round() == input_round);

            if let Some(proposal) = proposal {
                assert_eq!(proposal.value().id(), value_id);
                RoundInput::ProposalAndPolkaCurrent(proposal.clone())
            } else {
                RoundInput::PolkaAny
            }
        }

        RoundInput::PrecommitValue(value_id) => {
            let proposal = proposals.find(&value_id, |p| p.round() == input_round);

            if let Some(proposal) = proposal {
                assert_eq!(proposal.value().id(), value_id);
                RoundInput::ProposalAndPrecommitValue(proposal.clone())
            } else {
                RoundInput::PrecommitAny
            }
        }

        _ => input,
    }
}
