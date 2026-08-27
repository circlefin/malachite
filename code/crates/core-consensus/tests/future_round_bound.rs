/// Verify that `on_vote` drops votes whose round exceeds the future-round
/// lookahead before signature verification and WAL append.
use std::cell::Cell;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, State, MAX_FUTURE_ROUND_LOOKAHEAD,
};
use malachitebft_core_types::{Context, NilOrVal, Round, SignedVote, ValuePayload};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Signature, TestContext, Validator, ValidatorSet, ValueId,
};

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
}

impl Counters {
    fn new() -> Self {
        Self {
            verify_signature: Cell::new(0),
            wal_append_vote: Cell::new(0),
        }
    }

    fn reset(&self) {
        self.verify_signature.set(0);
        self.wal_append_vote.set(0);
    }

    fn handle(&self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;
        Ok(match effect {
            VerifySignature(_, _, r) => {
                self.verify_signature.set(self.verify_signature.get() + 1);
                r.resume_with(true)
            }
            WalAppend(_, entry, r) => {
                if matches!(entry, Input::Vote(_)) {
                    self.wal_append_vote.set(self.wal_append_vote.get() + 1);
                }
                r.resume_with(())
            }
            _ => Resume::Continue,
        })
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
