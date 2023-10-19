use crate::{Round, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    NewRound,                // Start a new round, not as proposer.
    NewRoundProposer(Value), // Start a new round and propose the Value.
    Proposal(Value, Round),  // Receive a proposal with possible polka round.
    ProposalInvalid,         // Receive an invalid proposal.
    PolkaAny,                // Receive +2/3 prevotes for anything.
    PolkaNil,                // Receive +2/3 prevotes for nil.
    PolkaValue(Value),       // Receive +2/3 prevotes for Value.
    PrecommitAny,            // Receive +2/3 precommits for anything.
    PrecommitValue(Value),   // Receive +2/3 precommits for Value.
    RoundSkip,               // Receive +1/3 votes from a higher round.
    TimeoutPropose,          // Timeout waiting for proposal.
    TimeoutPrevote,          // Timeout waiting for prevotes.
    TimeoutPrecommit,        // Timeout waiting for precommits.
}
