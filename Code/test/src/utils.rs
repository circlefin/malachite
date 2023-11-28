use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_common::{Round, Timeout, VotingPower};
use malachite_driver::{Event, Message, ProposerSelector, Validity};
use malachite_round::state::{RoundValue, State, Step};

use crate::{
    Address, Height, PrivateKey, Proposal, TestContext, Validator, ValidatorSet, Value, Vote,
};

#[derive(Copy, Clone, Debug, Default)]
pub struct RotateProposer;

impl ProposerSelector<TestContext> for RotateProposer {
    fn select_proposer(&self, round: Round, validator_set: &ValidatorSet) -> Address {
        let proposer_index = round.as_i64() as usize % validator_set.validators.len();
        validator_set.validators[proposer_index].address
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FixedProposer {
    proposer: Address,
}

impl FixedProposer {
    pub fn new(proposer: Address) -> Self {
        Self { proposer }
    }
}

impl ProposerSelector<TestContext> for FixedProposer {
    fn select_proposer(&self, _round: Round, _validator_set: &ValidatorSet) -> Address {
        self.proposer
    }
}

pub fn make_validators<const N: usize>(
    voting_powers: [VotingPower; N],
) -> [(Validator, PrivateKey); N] {
    let mut rng = StdRng::seed_from_u64(0x42);

    let mut validators = Vec::with_capacity(N);

    for vp in voting_powers {
        let sk = PrivateKey::generate(&mut rng);
        let val = Validator::new(sk.public_key(), vp);
        validators.push((val, sk));
    }

    validators.try_into().expect("N validators")
}

pub fn new_round_event(round: Round) -> Event<TestContext> {
    Event::NewRound(Height::new(1), round)
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
    Some(Message::Propose(proposal))
}

pub fn proposal_event(
    round: Round,
    value: Value,
    locked_round: Round,
    validity: Validity,
) -> Event<TestContext> {
    let proposal = Proposal::new(Height::new(1), round, value, locked_round);
    Event::Proposal(proposal, validity)
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

pub fn prevote_event(addr: &Address, sk: &PrivateKey) -> Event<TestContext> {
    let value = Value::new(9999);

    Event::Vote(
        Vote::new_prevote(Height::new(1), Round::new(0), Some(value.id()), *addr).signed(sk),
    )
}

pub fn prevote_event_at(round: Round, addr: &Address, sk: &PrivateKey) -> Event<TestContext> {
    let value = Value::new(9999);

    Event::Vote(Vote::new_prevote(Height::new(1), round, Some(value.id()), *addr).signed(sk))
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
) -> Event<TestContext> {
    Event::Vote(Vote::new_precommit(Height::new(1), round, Some(value.id()), *addr).signed(sk))
}

pub fn decide_message(round: Round, value: Value) -> Option<Message<TestContext>> {
    Some(Message::Decide(round, value))
}

pub fn start_propose_timer_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::propose(round)))
}

pub fn timeout_propose_event(round: Round) -> Event<TestContext> {
    Event::TimeoutElapsed(Timeout::propose(round))
}

pub fn start_prevote_timer_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::prevote(round)))
}

pub fn timeout_prevote_event(round: Round) -> Event<TestContext> {
    Event::TimeoutElapsed(Timeout::prevote(round))
}

pub fn start_precommit_timer_msg(round: Round) -> Option<Message<TestContext>> {
    Some(Message::ScheduleTimeout(Timeout::precommit(round)))
}

pub fn timeout_precommit_event(round: Round) -> Event<TestContext> {
    Event::TimeoutElapsed(Timeout::precommit(round))
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
