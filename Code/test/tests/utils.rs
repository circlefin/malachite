use malachite_common::{Round, Timeout};
use malachite_driver::{Event, Message, Validity};
use malachite_round::state::{RoundValue, State, Step};
use malachite_test::{Address, Height, PrivateKey, Proposal, TestContext, Value, Vote};

pub fn new_round_event(round: Round) -> Option<Event<TestContext>> {
    Some(Event::NewRound(Height::new(1), round))
}

pub fn new_round_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::NewRound(Height::new(1), round))
}

pub fn proposal_msg(
    round: Round,
    value: Value,
    locked_round: Round,
) -> Option<Message<TestContext>> {
    let proposal = Proposal::new(Height::new(1), round, value, locked_round);
    Some(Message::Propose(proposal.clone()))
}

pub fn proposal_event(
    round: Round,
    value: Value,
    locked_round: Round,
) -> Option<Event<TestContext>> {
    let proposal = Proposal::new(Height::new(1), round, value, locked_round);
    Some(Event::Proposal(proposal.clone(), Validity::Valid))
}

pub fn prevote_msg(round: Round, addr: &Address, sk: &PrivateKey) -> Option<Message<TestContext>> {
    let value = Value::new(9999);

    Some(Message::Vote(
        Vote::new_prevote(Height::new(1), round, Some(value.id()), *addr).signed(sk),
    ))
}

pub fn prevote_nil_msg(
    round: Round,
    addr: &Address,
    sk: &PrivateKey,
) -> Option<Message<TestContext>> {
    Some(Message::Vote(
        Vote::new_prevote(Height::new(1), round, None, *addr).signed(sk),
    ))
}

pub fn prevote_event(addr: &Address, sk: &PrivateKey) -> Option<Event<TestContext>> {
    let value = Value::new(9999);

    Some(Event::Vote(
        Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), *addr).signed(sk),
    ))
}

pub fn precommit_msg(
    round: Round,
    value: Value,
    addr: &Address,
    sk: &PrivateKey,
) -> Option<Message<TestContext>> {
    Some(Message::Vote(
        Vote::new_precommit(Height::new(1), round, Some(value.id()), *addr).signed(sk),
    ))
}

pub fn precommit_nil_msg(addr: &Address, sk: &PrivateKey) -> Option<Message<TestContext>> {
    Some(Message::Vote(
        Vote::new_precommit(Height::new(1), Round::new(0), None, *addr).signed(sk),
    ))
}

pub fn precommit_event(
    round: Round,
    value: Value,
    addr: &Address,
    sk: &PrivateKey,
) -> Option<Event<TestContext>> {
    Some(Event::Vote(
        Vote::new_precommit(Height::new(1), round, Some(value.id()), *addr).signed(sk),
    ))
}

pub fn decide_message(round: Round, value: Value) -> Option<Message<TestContext>> {
    Some(Message::Decide(round, value))
}

pub fn timeout_propose_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::propose(round)))
}

pub fn timeout_propose_fire_event(round: Round) -> Option<Event<TestContext>> {
    Some(Event::TimeoutElapsed(Timeout::propose(round)))
}

pub fn timeout_prevote_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::prevote(round)))
}

pub fn timeout_prevote_fire_event(round: Round) -> Option<Event<TestContext>> {
    Some(Event::TimeoutElapsed(Timeout::prevote(round)))
}

pub fn timeout_precommit_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::precommit(round)))
}

pub fn timeout_precommit_fire_event(round: Round) -> Option<Event<TestContext>> {
    Some(Event::TimeoutElapsed(Timeout::precommit(round)))
}

pub fn propose_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Propose,
        proposal: None,
        locked: None,
        valid: None,
    }
}

pub fn propose_state_with_proposal_and_valid(
    state_round: Round,
    valid_round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    // TODO - set_valid doesn't work because the valid round is set to state round
    // we need to set it to something different.
    // propose_state(round)
    //     .set_proposal(proposal.clone())
    //     .set_valid(proposal.value)
    State {
        height: Height::new(1),
        round: state_round,
        step: Step::Propose,
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: valid_round,
        }),
        locked: None,
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
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
    }
}

pub fn prevote_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        proposal: None,
        locked: None,
        valid: None,
    }
}

pub fn prevote_state_with_proposal(round: Round, proposal: Proposal) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        proposal: Some(proposal.clone()),
        valid: None,
        locked: None,
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
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: valid_round,
        }),
        locked: None,
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
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
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
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
    }
}

pub fn precommit_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Precommit,
        proposal: None,
        locked: None,
        valid: None,
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
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: valid_round,
        }),
        locked: None,
    }
}

pub fn new_round(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::NewRound,
        proposal: None,
        valid: None,
        locked: None,
    }
}

pub fn new_round_with_proposal_and_valid(round: Round, proposal: Proposal) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::NewRound,
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
        locked: None,
    }
}

pub fn new_round_with_proposal_and_locked_and_valid(
    round: Round,
    proposal: Proposal,
) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::NewRound,
        proposal: Some(proposal.clone()),
        valid: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.clone().value,
            round: Round::new(0),
        }),
    }
}

pub fn decided_state(round: Round, _value: Value) -> State<TestContext> {
    State {
        // TODO add decided, remove proposal
        height: Height::new(1),
        round,
        step: Step::Commit,
        proposal: None,
        valid: None,
        locked: None,
    }
}
