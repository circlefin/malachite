#![allow(clippy::needless_update)]

use malachitebft_core_state_machine::state::{RoundValue, State, Step};
use malachitebft_core_types::{NilOrVal, Round, SignedProposal, SignedVote, Timeout, Validity};
use malachitebft_test::{Address, Height, Proposal, Signature, TestContext, Value, Vote};

use informalsystems_malachitebft_core_driver::{Input, Output};

pub fn new_round_input(round: Round, proposer: Address) -> Input<TestContext> {
    Input::NewRound(Height::new(1), round, proposer)
}

pub fn new_round_output(round: Round) -> Output<TestContext> {
    Output::NewRound(Height::new(1), round)
}

pub fn proposal_output(
    round: Round,
    value: Value,
    locked_round: Round,
    address: Address,
) -> Output<TestContext> {
    let proposal = Proposal::new(Height::new(1), round, value, locked_round, address);
    Output::Propose(proposal)
}

pub fn proposal_input(
    round: Round,
    value: Value,
    locked_round: Round,
    validity: Validity,
    address: Address,
) -> Input<TestContext> {
    let proposal = Proposal::new(Height::new(1), round, value, locked_round, address);
    Input::Proposal(SignedProposal::new(proposal, Signature::test()), validity)
}

pub fn prevote_output(round: Round, value: Value, addr: &Address) -> Output<TestContext> {
    Output::Vote(Vote::new_prevote(
        Height::new(1),
        round,
        NilOrVal::Val(value.id()),
        *addr,
    ))
}

pub fn prevote_nil_output(round: Round, addr: &Address) -> Output<TestContext> {
    Output::Vote(Vote::new_prevote(
        Height::new(1),
        round,
        NilOrVal::Nil,
        *addr,
    ))
}

pub fn prevote_input(value: Value, addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_prevote(
            Height::new(1),
            Round::new(0),
            NilOrVal::Val(value.id()),
            *addr,
        ),
        Signature::test(),
    ))
}

pub fn prevote_nil_input(addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(0), NilOrVal::Nil, *addr),
        Signature::test(),
    ))
}

pub fn prevote_input_at(round: Round, value: Value, addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_prevote(Height::new(1), round, NilOrVal::Val(value.id()), *addr),
        Signature::test(),
    ))
}

pub fn precommit_output(round: Round, value: Value, addr: &Address) -> Output<TestContext> {
    Output::Vote(Vote::new_precommit(
        Height::new(1),
        round,
        NilOrVal::Val(value.id()),
        *addr,
    ))
}

pub fn precommit_nil_output(round: Round, addr: &Address) -> Output<TestContext> {
    Output::Vote(Vote::new_precommit(
        Height::new(1),
        round,
        NilOrVal::Nil,
        *addr,
    ))
}

pub fn precommit_input(round: Round, value: Value, addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_precommit(Height::new(1), round, NilOrVal::Val(value.id()), *addr),
        Signature::test(),
    ))
}

pub fn precommit_nil_input(round: Round, addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_precommit(Height::new(1), round, NilOrVal::Nil, *addr),
        Signature::test(),
    ))
}

pub fn precommit_input_at(round: Round, value: Value, addr: &Address) -> Input<TestContext> {
    Input::Vote(SignedVote::new(
        Vote::new_precommit(Height::new(1), round, NilOrVal::Val(value.id()), *addr),
        Signature::test(),
    ))
}

pub fn decide_output(round: Round, proposal: Proposal) -> Output<TestContext> {
    Output::Decide(round, proposal)
}

pub fn start_propose_timer_output(round: Round) -> Output<TestContext> {
    Output::ScheduleTimeout(Timeout::propose(round))
}

pub fn timeout_propose_input(round: Round) -> Input<TestContext> {
    Input::TimeoutElapsed(Timeout::propose(round))
}

pub fn start_prevote_timer_output(round: Round) -> Output<TestContext> {
    Output::ScheduleTimeout(Timeout::prevote(round))
}

pub fn timeout_prevote_input(round: Round) -> Input<TestContext> {
    Input::TimeoutElapsed(Timeout::prevote(round))
}

pub fn start_precommit_timer_output(round: Round) -> Output<TestContext> {
    Output::ScheduleTimeout(Timeout::precommit(round))
}

pub fn timeout_precommit_input(round: Round) -> Input<TestContext> {
    Input::TimeoutElapsed(Timeout::precommit(round))
}

pub fn propose_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Propose,
        ..Default::default()
    }
}

pub fn propose_state_with_proposal_and_valid(
    state_round: Round,
    valid_round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round: state_round,
        step: Step::Propose,
        valid: Some(RoundValue {
            value: proposal.value,
            round: valid_round,
        }),
        ..Default::default()
    }
}

pub fn propose_state_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Propose,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: proposal.round,
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: proposal.round,
        }),
        ..Default::default()
    }
}

pub fn prevote_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        ..Default::default()
    }
}

pub fn prevote_state_with_proposal_and_valid(
    state_round: Round,
    valid_round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round: state_round,
        step: Step::Prevote,
        valid: Some(RoundValue {
            value: proposal.value,
            round: valid_round,
        }),
        ..Default::default()
    }
}

pub fn prevote_state_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        ..Default::default()
    }
}

pub fn prevote_state_with_matching_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: proposal.round,
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: proposal.round,
        }),
        ..Default::default()
    }
}

pub fn precommit_state_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Precommit,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: proposal.round,
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: proposal.round,
        }),
        ..Default::default()
    }
}

pub fn precommit_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Precommit,
        ..Default::default()
    }
}

pub fn precommit_state_with_proposal_and_valid(
    state_round: Round,
    valid_round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round: state_round,
        step: Step::Precommit,
        valid: Some(RoundValue {
            value: proposal.value,
            round: valid_round,
        }),
        ..Default::default()
    }
}

pub fn new_round(round: Round) -> State<TestContext> {
    State::new(Height::new(1), round)
}

pub fn new_round_with_proposal_and_valid(round: Round, proposal: Proposal) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Unstarted,
        valid: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        ..Default::default()
    }
}

pub fn new_round_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Unstarted,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: proposal.round,
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: proposal.round,
        }),
        ..Default::default()
    }
}

pub fn decided_state(round: Round, value: Value) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Commit,
        decision: Some(value),
        ..Default::default()
    }
}

pub fn decided_state_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Commit,
        valid: Some(RoundValue {
            value: proposal.value.clone(),
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value.clone(),
            round: Round::new(0),
        }),
        decision: Some(proposal.value),
        ..Default::default()
    }
}
