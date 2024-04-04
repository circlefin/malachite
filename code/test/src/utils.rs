use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_common::{Context, NilOrVal, Round, Timeout, VotingPower};
use malachite_driver::{Input, Output, Validity};
use malachite_round::state::{RoundValue, State, Step};

use crate::{
    Address, Height, PrivateKey, Proposal, TestContext, Validator, ValidatorSet, Value, Vote,
};

/// Defines how to select a proposer amongst a validator set for a given round.
pub trait ProposerSelector<Ctx>
where
    Self: Send + Sync,
    Ctx: Context,
{
    /// Select a proposer from the given validator set for the given round.
    ///
    /// This function is called at the beginning of each round to select the proposer for that
    /// round. The proposer is responsible for proposing a value for the round.
    ///
    /// # Important
    /// This function must be deterministic!
    /// For a given round and validator set, it must always return the same proposer.
    fn select_proposer(
        &self,
        height: Ctx::Height,
        round: Round,
        validator_set: &Ctx::ValidatorSet,
    ) -> Ctx::Address;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct RotateProposer;

impl ProposerSelector<TestContext> for RotateProposer {
    fn select_proposer(
        &self,
        height: Height,
        round: Round,
        validator_set: &ValidatorSet,
    ) -> Address {
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let height = height.as_u64() as usize;
        let round = round.as_i64() as usize;

        let proposer_index = (height - 1 + round) % validator_set.validators.len();
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
    fn select_proposer(
        &self,
        _height: Height,
        _round: Round,
        _validator_set: &ValidatorSet,
    ) -> Address {
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
    Input::Proposal(proposal, validity)
}

pub fn prevote_output(round: Round, addr: &Address) -> Output<TestContext> {
    let value = Value::new(9999);

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

pub fn prevote_input(addr: &Address) -> Input<TestContext> {
    let value = Value::new(9999);

    Input::Vote(Vote::new_prevote(
        Height::new(1),
        Round::new(0),
        NilOrVal::Val(value.id()),
        *addr,
    ))
}

pub fn prevote_nil_input(addr: &Address) -> Input<TestContext> {
    Input::Vote(Vote::new_prevote(
        Height::new(1),
        Round::new(0),
        NilOrVal::Nil,
        *addr,
    ))
}

pub fn prevote_input_at(round: Round, addr: &Address) -> Input<TestContext> {
    let value = Value::new(9999);

    Input::Vote(Vote::new_prevote(
        Height::new(1),
        round,
        NilOrVal::Val(value.id()),
        *addr,
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
    Input::Vote(Vote::new_precommit(
        Height::new(1),
        round,
        NilOrVal::Val(value.id()),
        *addr,
    ))
}

pub fn decide_output(round: Round, value: Value) -> Output<TestContext> {
    Output::Decide(round, value)
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
        locked: None,
        valid: None,
        decision: None,
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
        locked: None,
        decision: None,
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
            value: proposal.value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        decision: None,
    }
}

pub fn prevote_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Prevote,
        locked: None,
        valid: None,
        decision: None,
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
        locked: None,
        decision: None,
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
            value: proposal.value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        decision: None,
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
            value: proposal.value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        decision: None,
    }
}

pub fn precommit_state(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Precommit,
        locked: None,
        valid: None,
        decision: None,
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
        locked: None,
        decision: None,
    }
}

pub fn new_round(round: Round) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Unstarted,
        valid: None,
        locked: None,
        decision: None,
    }
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
        locked: None,
        decision: None,
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
            value: proposal.value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        decision: None,
    }
}

pub fn decided_state(round: Round, value: Value) -> State<TestContext> {
    State {
        height: Height::new(1),
        round,
        step: Step::Commit,
        valid: None,
        locked: None,
        decision: Some(value),
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
            value: proposal.value,
            round: Round::new(0),
        }),
        locked: Some(RoundValue {
            value: proposal.value,
            round: Round::new(0),
        }),
        decision: Some(proposal.value),
    }
}
