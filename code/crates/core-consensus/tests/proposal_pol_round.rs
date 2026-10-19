//! Consensus drops a proposal from the network when its pol_round is not lower than its round.
//! Proposals replayed from the write-ahead log are kept.
//!
//! A correct proposer sends either no pol_round or a pol_round lower than the round (L19).
//! Without the check, a Byzantine proposer can send some validators a proposal whose pol_round
//! equals its round. If such a validator then sees a polka for that value before it sees a
//! quorum of prevotes for any value, it stays in the prevote step. It neither precommits (L36)
//! nor starts the prevote timer (L34).

use std::vec::Vec;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, SignedConsensusMsg, State,
    ValuePayload,
};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, Round, SignedProposal, SignedVote, Timeout, VoteType,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

const X: u64 = 9999;
const Y: u64 = 42;

#[derive(Default)]
struct Captured {
    wal: Vec<Input<TestContext>>,
    signed_votes: Vec<Vote>,
    published_votes: Vec<SignedVote<TestContext>>,
    timeouts: Vec<Timeout>,
}

fn handle_effect(
    effect: Effect<TestContext>,
    cap: &mut Captured,
) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        VerifyPolkaCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => {
            cap.signed_votes.push(vote.clone());
            r.resume_with(SignedVote::new(vote, Signature::test()))
        }
        SignProposal(proposal, r) => {
            r.resume_with(SignedProposal::new(proposal, Signature::test()))
        }
        ExtendVote(_, _, _, _, r) => r.resume_with(None),
        VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
        ScheduleTimeout(timeout, r) => {
            cap.timeouts.push(timeout);
            r.resume_with(())
        }
        WalAppend(_, input, r) => {
            cap.wal.push(input);
            r.resume_with(())
        }
        PublishConsensusMsg(msg, r) => {
            if let SignedConsensusMsg::Vote(vote) = msg {
                cap.published_votes.push(vote);
            }
            r.resume_with(())
        }
        _ => Resume::Continue,
    })
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    State::new(
        TestContext::new(),
        Height::new(1),
        ValidatorSet::new(validators.to_vec()),
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn start_height(validators: &[Validator]) -> Input<TestContext> {
    Input::StartHeight(
        Height::new(1),
        ValidatorSet::new(validators.to_vec()),
        false,
        None,
        Default::default(),
    )
}

fn proposal(proposer: Address, round: u32, value: u64, pol_round: Round) -> Input<TestContext> {
    Input::Proposal(SignedProposal::new(
        Proposal::new(
            Height::new(1),
            Round::new(round),
            Value::new(value),
            pol_round,
            proposer,
        ),
        Signature::test(),
    ))
}

/// The vote value for `value`, where `None` stands for nil.
fn vote_value(value: Option<u64>) -> NilOrVal<ValueId> {
    value.map_or(NilOrVal::Nil, |v| NilOrVal::Val(Value::new(v).id()))
}

fn prevote_at(addr: Address, round: u32, value: Option<u64>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(round), vote_value(value), addr),
        Signature::test(),
    )
}

fn prevote(addr: Address, value: u64) -> SignedVote<TestContext> {
    prevote_at(addr, 0, Some(value))
}

fn polka_certificate(voters: &[Address], value: u64) -> Input<TestContext> {
    Input::PolkaCertificate(PolkaCertificate::new(
        Height::new(1),
        Round::new(0),
        Value::new(value).id(),
        voters.iter().map(|addr| prevote(*addr, value)).collect(),
    ))
}

fn drive(state: &mut State<TestContext>, inputs: Vec<Input<TestContext>>) -> Captured {
    drive_with(state, &Metrics::new(), inputs)
}

/// Like [`drive`], but shares `metrics` across calls, which step tracking needs.
fn drive_with(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    inputs: Vec<Input<TestContext>>,
) -> Captured {
    let mut cap = Captured::default();
    for input in inputs {
        let result: Result<(), Error<TestContext>> = process!(
            input: input,
            state: state,
            metrics: metrics,
            with: e => handle_effect(e, &mut cap)
        );
        result.expect("consensus input should process successfully");
    }
    cap
}

fn proposals_in_wal(cap: &Captured) -> Vec<&SignedProposal<TestContext>> {
    cap.wal
        .iter()
        .filter_map(|input| match input {
            Input::Proposal(proposal) => Some(proposal),
            _ => None,
        })
        .collect()
}

/// Whether the node published a round 0 vote of type `typ` for `value`, where `None` is nil.
fn published(cap: &Captured, typ: VoteType, value: Option<u64>) -> bool {
    let expected = vote_value(value);
    cap.published_votes.iter().any(|vote| {
        vote.message.typ == typ
            && vote.message.round == Round::new(0)
            && vote.message.value == expected
    })
}

fn proposal_evidence_count(state: &State<TestContext>, addr: Address) -> usize {
    state
        .driver
        .proposals()
        .evidence()
        .get(&addr)
        .map(|v: &Vec<_>| v.len())
        .unwrap_or(0)
}

fn validators(weights: [u64; 4]) -> Vec<Validator> {
    make_validators(weights)
        .into_iter()
        .map(|(v, _)| v)
        .collect()
}

/// The proposer of `round` at height 1.
fn proposer(validators: &[Validator], round: u32) -> Address {
    TestContext::new()
        .select_proposer(
            &ValidatorSet::new(validators.to_vec()),
            Height::new(1),
            Round::new(round),
        )
        .address
}

/// The validators that are not in `excluded`, in the order of `validators`.
fn others(validators: &[Validator], excluded: &[Address]) -> Vec<Address> {
    validators
        .iter()
        .map(|v| v.address)
        .filter(|addr| !excluded.contains(addr))
        .collect()
}

#[test]
fn proposal_with_pol_round_equal_to_round_is_dropped() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let mut state = make_state(&validators, others(&validators, &[p])[0]);

    let cap = drive(
        &mut state,
        vec![start_height(&validators), proposal(p, 0, X, Round::new(0))],
    );

    assert!(proposals_in_wal(&cap).is_empty());
    assert!(state
        .full_proposal_at_round_and_value(&Height::new(1), Round::new(0), &Value::new(X))
        .is_none());
}

#[test]
fn proposal_with_pol_round_above_round_is_dropped() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let mut state = make_state(&validators, others(&validators, &[p])[0]);

    let cap = drive(
        &mut state,
        vec![start_height(&validators), proposal(p, 0, X, Round::new(1))],
    );

    assert!(proposals_in_wal(&cap).is_empty());
    assert!(state
        .full_proposal_at_round_and_value(&Height::new(1), Round::new(0), &Value::new(X))
        .is_none());
}

// The check compares pol_round with the round of the proposal, not with the current round. Here
// the node is in round 0 and receives two round 1 proposals.
#[test]
fn future_round_proposal_is_checked_against_its_own_round() {
    let validators = validators([1, 1, 1, 1]);
    let p0 = proposer(&validators, 0);
    let p1 = proposer(&validators, 1);
    let mut state = make_state(&validators, others(&validators, &[p0, p1])[0]);

    let cap = drive(
        &mut state,
        vec![
            start_height(&validators),
            proposal(p1, 1, X, Round::new(0)),
            proposal(p1, 1, Y, Round::new(1)),
        ],
    );

    assert_eq!(proposals_in_wal(&cap).len(), 1);
    assert!(state
        .full_proposal_at_round_and_value(&Height::new(1), Round::new(1), &Value::new(X))
        .is_some());
    assert!(state
        .full_proposal_at_round_and_value(&Height::new(1), Round::new(1), &Value::new(Y))
        .is_none());
    assert_eq!(
        proposal_evidence_count(&state, p1),
        0,
        "the second proposal must be dropped, not kept as equivocation evidence"
    );
}

// The same check for a proposal from an earlier round. Two round 1 prevotes move the node to
// round 1 (L55). It then receives a round 0 proposal whose pol_round equals its round.
#[test]
fn past_round_proposal_is_checked_against_its_own_round() {
    let validators = validators([1, 1, 1, 1]);
    let p0 = proposer(&validators, 0);
    let rest = others(&validators, &[p0]);
    let (me, v2, v3) = (rest[0], rest[1], rest[2]);
    let mut state = make_state(&validators, me);

    let metrics = Metrics::new();
    drive_with(
        &mut state,
        &metrics,
        vec![
            start_height(&validators),
            Input::Vote(prevote_at(v2, 1, None)),
            Input::Vote(prevote_at(v3, 1, None)),
        ],
    );
    assert_eq!(state.round(), Round::new(1));

    let cap = drive_with(
        &mut state,
        &metrics,
        vec![proposal(p0, 0, X, Round::new(0))],
    );

    assert!(proposals_in_wal(&cap).is_empty());
    assert!(state
        .full_proposal_at_round_and_value(&Height::new(1), Round::new(0), &Value::new(X))
        .is_none());
}

// A dropped proposal must not block a later proposal for the same value. The well-formed
// proposal is stored, and a polka for its value leads to a precommit (L36).
#[test]
fn well_formed_proposal_after_dropped_one_leads_to_precommit() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let rest = others(&validators, &[p]);
    let (me, v2, v3) = (rest[0], rest[1], rest[2]);
    let mut state = make_state(&validators, me);

    let cap = drive(
        &mut state,
        vec![
            start_height(&validators),
            proposal(p, 0, X, Round::new(0)),
            proposal(p, 0, X, Round::Nil),
            Input::Vote(prevote(p, X)),
            Input::Vote(prevote(v2, X)),
            Input::Vote(prevote(v3, X)),
        ],
    );

    let logged = proposals_in_wal(&cap);
    assert_eq!(logged.len(), 1, "got {logged:?}");
    assert_eq!(logged[0].message.pol_round, Round::Nil);
    assert!(
        published(&cap, VoteType::Prevote, Some(X)),
        "the node must prevote X, got {:?}",
        cap.published_votes
    );
    assert!(
        published(&cap, VoteType::Precommit, Some(X)),
        "the node must precommit X, got {:?}",
        cap.published_votes
    );
}

// The same as the test above, but the polka arrives as a polka certificate.
#[test]
fn well_formed_proposal_after_dropped_one_leads_to_precommit_with_polka_certificate() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let rest = others(&validators, &[p]);
    let (me, v2, v3) = (rest[0], rest[1], rest[2]);
    let mut state = make_state(&validators, me);

    let cap = drive(
        &mut state,
        vec![
            start_height(&validators),
            proposal(p, 0, X, Round::new(0)),
            proposal(p, 0, X, Round::Nil),
            polka_certificate(&[p, v2, v3], X),
        ],
    );

    assert!(
        published(&cap, VoteType::Precommit, Some(X)),
        "the node must precommit X, got {:?}",
        cap.published_votes
    );
}

// An earlier version stored the first of two proposals for X, the one with pol_round 0. The
// driver ignored it, so the node prevoted nil when the propose timer fired. The write-ahead log
// holds both proposals, the timeout and the nil prevote.
//
// Replay must derive the same nil prevote and reuse the recorded one. If replay dropped the
// first proposal, the second one would lead to a prevote for X (L22). Replay would then stop
// with a `ReplayDivergence` error.
#[test]
fn replay_of_old_wal_with_pol_round_equal_to_round_reuses_the_recorded_prevote() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let me = others(&validators, &[p])[0];
    let mut state = make_state(&validators, me);
    let metrics = Metrics::new();

    let wal = vec![
        proposal(p, 0, X, Round::new(0)),
        proposal(p, 0, X, Round::Nil),
        Input::TimeoutElapsed(Timeout::propose(Round::new(0))),
        Input::Vote(prevote_at(me, 0, None)),
    ];

    drive_with(&mut state, &metrics, vec![start_height(&validators)]);
    state.index_wal_entries(wal.iter());
    let cap = drive_with(&mut state, &metrics, wal);
    state.reset_entries_index();

    assert!(
        cap.signed_votes.is_empty(),
        "replay must reuse the recorded prevote instead of signing a new one, signed {:?}",
        cap.signed_votes
    );
    assert!(
        published(&cap, VoteType::Prevote, None),
        "replay must publish the recorded nil prevote, got {:?}",
        cap.published_votes
    );
}

// Equal weights. The proposer P sends the node two proposals: one for X with pol_round 0, and one
// for Y. The node prevotes Y. P's polka certificate for X then reaches the node before the last
// honest prevote. Applying the certificate discards the vote keeper's outputs, so the certificate
// itself must start the prevote timer.
#[test]
fn certificate_race_with_pol_round_equal_to_round_starts_prevote_timer() {
    let validators = validators([1, 1, 1, 1]);
    let p = proposer(&validators, 0);
    let rest = others(&validators, &[p]);
    let (me, v2, v3) = (rest[0], rest[1], rest[2]);
    let mut state = make_state(&validators, me);
    let metrics = Metrics::new();

    let cap = drive_with(
        &mut state,
        &metrics,
        vec![
            start_height(&validators),
            proposal(p, 0, X, Round::new(0)),
            proposal(p, 0, Y, Round::Nil),
            Input::Vote(prevote(v2, X)),
            polka_certificate(&[p, v2, v3], X),
        ],
    );

    assert!(
        published(&cap, VoteType::Prevote, Some(Y)),
        "the node must prevote Y, got {:?}",
        cap.published_votes
    );
    assert!(
        cap.timeouts.contains(&Timeout::prevote(Round::new(0))),
        "the node must start the prevote timer, got {:?}",
        cap.timeouts
    );

    let cap = drive_with(
        &mut state,
        &metrics,
        vec![Input::TimeoutElapsed(Timeout::prevote(Round::new(0)))],
    );
    assert!(
        published(&cap, VoteType::Precommit, None),
        "the node must precommit nil, got {:?}",
        cap.published_votes
    );
}

// Uneven weights and no certificate. The proposer and the node have weight 1. The other two
// validators have weight 2. A quorum is 5 of 6. The node's prevote for Y, P's prevote for X, and
// one weight-2 prevote for X come first. The last prevote for X then completes the polka for X and
// the quorum for any value at the same time. The vote keeper reports only the polka for X.
#[test]
fn single_prevote_completing_both_quorums_with_pol_round_equal_to_round_starts_prevote_timer() {
    let validators = validators([1, 2, 2, 1]);
    let p = proposer(&validators, 0);
    let weight = |addr: Address| {
        validators
            .iter()
            .find(|v| v.address == addr)
            .map(|v| v.voting_power)
            .expect("known validator")
    };
    assert_eq!(weight(p), 1, "the test needs a proposer with weight 1");

    let rest = others(&validators, &[p]);
    let me = *rest
        .iter()
        .find(|addr| weight(**addr) == 1)
        .expect("a validator with weight 1 other than the proposer");
    let [heavy_a, heavy_b]: [Address; 2] = rest
        .iter()
        .copied()
        .filter(|addr| weight(*addr) == 2)
        .collect::<Vec<_>>()
        .try_into()
        .expect("two validators with weight 2");
    let mut state = make_state(&validators, me);
    let metrics = Metrics::new();

    let cap = drive_with(
        &mut state,
        &metrics,
        vec![
            start_height(&validators),
            proposal(p, 0, X, Round::new(0)),
            proposal(p, 0, Y, Round::Nil),
            Input::Vote(prevote(p, X)),
            Input::Vote(prevote(heavy_a, X)),
        ],
    );
    assert!(
        published(&cap, VoteType::Prevote, Some(Y)),
        "the node must prevote Y, got {:?}",
        cap.published_votes
    );
    assert!(
        !cap.timeouts.contains(&Timeout::prevote(Round::new(0))),
        "the node must not start the prevote timer yet, got {:?}",
        cap.timeouts
    );

    let cap = drive_with(&mut state, &metrics, vec![Input::Vote(prevote(heavy_b, X))]);
    assert!(
        cap.timeouts.contains(&Timeout::prevote(Round::new(0))),
        "the node must start the prevote timer, got {:?}",
        cap.timeouts
    );
}
