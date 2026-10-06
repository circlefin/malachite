/// Verify that `on_vote`, `on_proposal`, and `on_proposed_value` handle the
/// future-round lookahead consistently: votes and proposals are dropped
/// before signature verification and WAL append once their round exceeds
/// the lookahead, while proposed values remain unbound regardless of origin.
use std::cell::Cell;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, State, MAX_FUTURE_ROUND_LOOKAHEAD,
};
use malachitebft_core_types::{
    Context, NilOrVal, Round, SignedProposal, SignedVote, Validity, ValueOrigin, ValuePayload,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
};

use arc_malachitebft_core_consensus::ProposedValue;

fn run(r: Result<(), Error<TestContext>>) {
    drop(r);
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs,
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        500,
    )
}

fn signed_prevote_from(
    ctx: &TestContext,
    height: Height,
    round: Round,
    addr: Address,
) -> SignedVote<TestContext> {
    let vote = ctx.new_prevote(height, round, NilOrVal::Val(ValueId::new(1)), addr);
    SignedVote::new(vote, Signature::test())
}

struct Counters {
    verify_signature: Cell<u32>,
    wal_append_vote: Cell<u32>,
    wal_append_proposal: Cell<u32>,
    wal_append_proposed_value: Cell<u32>,
}

impl Counters {
    fn new() -> Self {
        Self {
            verify_signature: Cell::new(0),
            wal_append_vote: Cell::new(0),
            wal_append_proposal: Cell::new(0),
            wal_append_proposed_value: Cell::new(0),
        }
    }

    fn reset(&self) {
        self.verify_signature.set(0);
        self.wal_append_vote.set(0);
        self.wal_append_proposal.set(0);
        self.wal_append_proposed_value.set(0);
    }

    fn handle(&self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;
        Ok(match effect {
            VerifySignature(_, _, r) => {
                self.verify_signature.set(self.verify_signature.get() + 1);
                r.resume_with(true)
            }
            WalAppend(_, entry, r) => {
                match &entry {
                    Input::Vote(_) => self.wal_append_vote.set(self.wal_append_vote.get() + 1),
                    Input::Proposal(_) => self
                        .wal_append_proposal
                        .set(self.wal_append_proposal.get() + 1),
                    Input::ProposedValue(_, _) => self
                        .wal_append_proposed_value
                        .set(self.wal_append_proposed_value.get() + 1),
                    _ => {}
                }
                r.resume_with(())
            }
            _ => Resume::Continue,
        })
    }
}

fn signed_proposal(height: Height, round: Round, proposer: Address) -> SignedProposal<TestContext> {
    SignedProposal::new(
        Proposal::new(height, round, Value::new(1), Round::Nil, proposer),
        Signature::test(),
    )
}

fn proposed_value(height: Height, round: Round, proposer: Address) -> ProposedValue<TestContext> {
    ProposedValue {
        height,
        round,
        valid_round: Round::Nil,
        proposer,
        value: Value::new(1),
        validity: Validity::Valid,
    }
}

/// A prevote whose round exceeds the future-round lookahead is dropped before
/// signature verification and WAL append.
#[test]
fn prevote_beyond_future_round_lookahead_is_dropped() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));
    counters.reset();

    let beyond_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let vote = signed_prevote_from(&ctx, height, beyond_ceiling, sender_addr);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "vote beyond the future-round lookahead must not be signature-verified"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "vote beyond the future-round lookahead must not be appended to the WAL"
    );
}

/// A prevote whose round is exactly at the future-round lookahead is accepted:
/// both signature verification and WAL append happen.
#[test]
fn prevote_at_future_round_lookahead_is_accepted() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));
    counters.reset();

    let at_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD);
    let vote = signed_prevote_from(&ctx, height, at_ceiling, sender_addr);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "vote at the future-round lookahead must be signature-verified"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        1,
        "vote at the future-round lookahead must be appended to the WAL"
    );
}

/// Advance `state` to `target` (a non-zero round) by feeding prevotes from
/// enough distinct validators to meet the skip-round threshold. `target` must
/// be within the future-round lookahead of round 0 so the skip votes are not
/// themselves dropped.
fn advance_to_round(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    ctx: &TestContext,
    height: Height,
    target: Round,
    voters: &[Address],
    counters: &Counters,
) {
    for addr in voters {
        let vote = signed_prevote_from(ctx, height, target, *addr);
        run(process!(
            input: Input::Vote(vote),
            state: state,
            metrics: metrics,
            with: effect => counters.handle(effect)
        ));
    }
}

/// The future-round ceiling slides with the consensus round. Once the node has
/// advanced past round 0, a vote at `current_round + MAX_FUTURE_ROUND_LOOKAHEAD`
/// is still accepted while a vote one round beyond it is dropped — neither of
/// which holds against a ceiling anchored at round 0.
#[test]
fn future_round_bound_slides_with_consensus_round() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));

    // Move off round 0: two validators' prevotes meet the f+1 skip-round
    // threshold and advance the node to round 3.
    let consensus_round = Round::new(3);
    advance_to_round(
        &mut state,
        &metrics,
        &ctx,
        height,
        consensus_round,
        &[validators[1].address, validators[2].address],
        &counters,
    );

    assert_eq!(state.round(), consensus_round);

    let current = consensus_round.as_u32().unwrap();

    // A vote at exactly the slid ceiling is accepted.
    counters.reset();
    let at_ceiling = Round::new(current + MAX_FUTURE_ROUND_LOOKAHEAD);
    let vote = signed_prevote_from(&ctx, height, at_ceiling, sender_addr);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "vote at the slid future-round ceiling must be signature-verified"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        1,
        "vote at the slid future-round ceiling must be appended to the WAL"
    );

    // A vote one round beyond the slid ceiling is dropped.
    counters.reset();
    let beyond_ceiling = Round::new(current + MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let vote = signed_prevote_from(&ctx, height, beyond_ceiling, sender_addr);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "vote beyond the slid future-round ceiling must not be signature-verified"
    );
    assert_eq!(
        counters.wal_append_vote.get(),
        0,
        "vote beyond the slid future-round ceiling must not be appended to the WAL"
    );
}

/// A proposal whose round exceeds the future-round lookahead is dropped before
/// signature verification and WAL append.
#[test]
fn proposal_beyond_future_round_lookahead_is_dropped() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));
    counters.reset();

    let beyond_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let proposer = *state.get_proposer(height, beyond_ceiling);
    let proposal = signed_proposal(height, beyond_ceiling, proposer);

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "proposal beyond the future-round lookahead must not be signature-verified"
    );
    assert_eq!(
        counters.wal_append_proposal.get(),
        0,
        "proposal beyond the future-round lookahead must not be appended to the WAL"
    );
}

/// A proposal whose round is exactly at the future-round lookahead is accepted.
#[test]
fn proposal_at_future_round_lookahead_is_accepted() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));
    counters.reset();

    let at_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD);
    let proposer = *state.get_proposer(height, at_ceiling);
    let proposal = signed_proposal(height, at_ceiling, proposer);

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "proposal at the future-round lookahead must be signature-verified"
    );
    assert_eq!(
        counters.wal_append_proposal.get(),
        1,
        "proposal at the future-round lookahead must be appended to the WAL"
    );
}

/// The future-round ceiling for proposals slides with the consensus round,
/// mirroring votes. Once the node has advanced past round 0, a proposal at
/// `current_round + MAX_FUTURE_ROUND_LOOKAHEAD` is still accepted while one
/// round beyond it is dropped.
#[test]
fn proposal_future_round_bound_slides_with_consensus_round() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));

    let consensus_round = Round::new(3);
    advance_to_round(
        &mut state,
        &metrics,
        &ctx,
        height,
        consensus_round,
        &[validators[1].address, validators[2].address],
        &counters,
    );

    assert_eq!(state.round(), consensus_round);

    let current = consensus_round.as_u32().unwrap();

    // A proposal at exactly the slid ceiling is accepted.
    counters.reset();
    let at_ceiling = Round::new(current + MAX_FUTURE_ROUND_LOOKAHEAD);
    let proposer = *state.get_proposer(height, at_ceiling);
    let proposal = signed_proposal(height, at_ceiling, proposer);

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "proposal at the slid future-round ceiling must be signature-verified"
    );
    assert_eq!(
        counters.wal_append_proposal.get(),
        1,
        "proposal at the slid future-round ceiling must be appended to the WAL"
    );

    // A proposal one round beyond the slid ceiling is dropped.
    counters.reset();
    let beyond_ceiling = Round::new(current + MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let proposer = *state.get_proposer(height, beyond_ceiling);
    let proposal = signed_proposal(height, beyond_ceiling, proposer);

    run(process!(
        input: Input::Proposal(proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "proposal beyond the slid future-round ceiling must not be signature-verified"
    );
    assert_eq!(
        counters.wal_append_proposal.get(),
        0,
        "proposal beyond the slid future-round ceiling must not be appended to the WAL"
    );
}

/// Proposed values are not subject to the future-round lookahead: WAL decode
/// always reconstructs them as Consensus origin, so a bound here would drop a
/// crash-recovered sync value. Both live origins must WAL-append.
#[test]
fn proposed_value_beyond_lookahead_is_kept_for_consensus_and_sync() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, false, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));
    counters.reset();

    let beyond_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let proposer = *state.get_proposer(height, beyond_ceiling);
    let value = proposed_value(height, beyond_ceiling, proposer);

    run(process!(
        input: Input::ProposedValue(value.clone(), ValueOrigin::Consensus),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.wal_append_proposed_value.get(),
        1,
        "consensus-origin proposed value (WAL-replay shape) must not be dropped by the lookahead bound"
    );

    counters.reset();
    run(process!(
        input: Input::ProposedValue(value, ValueOrigin::Sync),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.wal_append_proposed_value.get(),
        1,
        "sync-origin proposed value must not be dropped by the lookahead bound"
    );
}

/// While replaying the Write-Ahead Log, a vote beyond the future-round ceiling is
/// applied rather than dropped: the read path must accept every vote the write path
/// accepted, and a round certificate can place those votes arbitrarily far ahead of
/// the round replay starts from.
#[test]
fn vote_beyond_future_round_lookahead_is_accepted_while_replaying_wal() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let sender_addr = validators[1].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());
    let ctx = TestContext::new();

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, true, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));

    let beyond_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let vote = signed_prevote_from(&ctx, height, beyond_ceiling, sender_addr);
    let wal = [Input::Vote(vote.clone())];

    state.index_wal_entries(wal.iter());
    counters.reset();

    run(process!(
        input: Input::Vote(vote.clone()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "replayed vote beyond the future-round ceiling must be signature-verified"
    );
    assert!(
        state.driver.votes().has_vote(&vote),
        "replayed vote beyond the future-round ceiling must reach the vote keeper"
    );

    // Once replay completes the bound applies again to the same round.
    state.reset_entries_index();
    counters.reset();

    let vote = signed_prevote_from(&ctx, height, beyond_ceiling, validators[2].address);

    run(process!(
        input: Input::Vote(vote),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "once replay is over, a vote beyond the future-round ceiling must be dropped again"
    );
}

/// While replaying the Write-Ahead Log, a proposal beyond the future-round
/// ceiling is applied rather than dropped: the read path must accept every
/// proposal the write path accepted, and a round certificate can place those
/// proposals arbitrarily far ahead of the round replay starts from.
#[test]
fn proposal_beyond_future_round_lookahead_is_accepted_while_replaying_wal() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let counters = Counters::new();

    run(process!(
        input: Input::StartHeight(height, vs, true, None, Default::default()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(state.round(), Round::new(0));

    let beyond_ceiling = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 1);
    let proposer = *state.get_proposer(height, beyond_ceiling);
    let proposal = signed_proposal(height, beyond_ceiling, proposer);
    let wal = [Input::Proposal(proposal.clone())];

    state.index_wal_entries(wal.iter());
    counters.reset();

    run(process!(
        input: Input::Proposal(proposal.clone()),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        1,
        "replayed proposal beyond the future-round ceiling must be signature-verified"
    );
    assert!(
        state
            .full_proposal_at_round_and_value(&height, beyond_ceiling, &Value::new(1))
            .is_some(),
        "replayed proposal beyond the future-round ceiling must reach the proposal keeper"
    );

    // Once replay completes the bound applies again to the same round.
    state.reset_entries_index();
    counters.reset();

    let later_round = Round::new(MAX_FUTURE_ROUND_LOOKAHEAD + 2);
    let later_proposer = *state.get_proposer(height, later_round);
    let later_proposal = signed_proposal(height, later_round, later_proposer);

    run(process!(
        input: Input::Proposal(later_proposal),
        state: &mut state,
        metrics: &metrics,
        with: effect => counters.handle(effect)
    ));

    assert_eq!(
        counters.verify_signature.get(),
        0,
        "once replay is over, a proposal beyond the future-round ceiling must be dropped again"
    );
}
