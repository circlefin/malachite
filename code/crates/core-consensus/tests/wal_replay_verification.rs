//! WAL replay must reuse the messages it recorded, and must refuse to act when re-derivation
//! disagrees with them.
//!
//! Recovery re-runs the state machine over the recorded inputs, so the driver re-derives every
//! vote and proposal the node cast before the crash. The record of each such message comes
//! *after* the input that triggered it, so re-derivation always runs first — and, before this
//! was addressed, the re-derived message was signed afresh and published while the recorded one
//! was dropped by the votekeeper's dedup.
//!
//! Each test below runs a scenario live, capturing the exact inputs a real node would have
//! written to its log, and then replays that capture against a fresh state. The replay signer
//! hands back a signature the live run never produced, so "reused the record" and "signed again"
//! are told apart by the published signature as well as by the signer call count.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, RecordKind, Resumable, Resume,
    SignedConsensusMsg, State, ValuePayload,
};
use bytes::Bytes;
use malachitebft_core_types::{
    NilOrVal, Round, RoundCertificate, RoundCertificateType, SignedMessage, SignedProposal,
    SignedVote, Timeout, Validity, ValueOrigin, VoteExtensionPolicy, VoteType,
};
// The `Vote` and `Proposal` traits, for the accessors; the concrete types of the same name come
// from `malachitebft_test`.
use malachitebft_core_types::{Proposal as _, Vote as _};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Proposal, Signature, TestContext, Validator, ValidatorSet, Value, ValueId,
    Vote,
};

const HEIGHT: u64 = 1;

/// Signature produced by the signer during the live run, and therefore the one carried by every
/// record in the captured log.
fn live_signature() -> Signature {
    Signature::from_bytes([0xa1; 64])
}

/// Signature produced by the signer during replay. Seeing it on the wire means the node signed
/// again instead of reusing what it had recorded.
fn replay_signature() -> Signature {
    Signature::from_bytes([0xb2; 64])
}

/// Records what the node put in its log and what it sent to peers, and counts signer calls.
struct Harness {
    /// Signature this harness's signer hands back.
    signature: Signature,
    /// Vote extension this harness's application hands back, if any.
    extension: Option<&'static [u8]>,
    wal: RefCell<Vec<Input<TestContext>>>,
    published: RefCell<Vec<SignedConsensusMsg<TestContext>>>,
    extended_votes: Cell<u32>,
    signed_votes: Cell<u32>,
    signed_proposals: Cell<u32>,
}

impl Harness {
    fn new(signature: Signature) -> Self {
        Self {
            signature,
            extension: None,
            wal: RefCell::new(Vec::new()),
            published: RefCell::new(Vec::new()),
            extended_votes: Cell::new(0),
            signed_votes: Cell::new(0),
            signed_proposals: Cell::new(0),
        }
    }

    fn with_extension(mut self, extension: &'static [u8]) -> Self {
        self.extension = Some(extension);
        self
    }

    fn handle(&self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;

        Ok(match effect {
            VerifySignature(_, _, r) => r.resume_with(true),
            VerifyCommitCertificate(_, _, _, r)
            | VerifyPolkaCertificate(_, _, _, r)
            | VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
            VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
            ExtendVote(_, _, _, _, r) => {
                self.extended_votes.set(self.extended_votes.get() + 1);
                r.resume_with(
                    self.extension
                        .map(|ext| SignedMessage::new(Bytes::from_static(ext), Signature::test())),
                )
            }

            SignVote(vote, r) => {
                self.signed_votes.set(self.signed_votes.get() + 1);
                r.resume_with(SignedVote::new(vote, self.signature))
            }

            SignProposal(proposal, r) => {
                self.signed_proposals.set(self.signed_proposals.get() + 1);
                r.resume_with(SignedProposal::new(proposal, self.signature))
            }

            WalAppend(_, entry, r) => {
                self.wal.borrow_mut().push(entry);
                r.resume_with(())
            }

            PublishConsensusMsg(msg, r) => {
                self.published.borrow_mut().push(msg);
                r.resume_with(())
            }

            _ => Resume::Continue,
        })
    }

    fn published_votes(&self) -> Vec<SignedVote<TestContext>> {
        self.published
            .borrow()
            .iter()
            .filter_map(|msg| match msg {
                SignedConsensusMsg::Vote(vote) => Some(vote.clone()),
                SignedConsensusMsg::Proposal(_) => None,
            })
            .collect()
    }

    fn published_proposals(&self) -> Vec<SignedProposal<TestContext>> {
        self.published
            .borrow()
            .iter()
            .filter_map(|msg| match msg {
                SignedConsensusMsg::Proposal(proposal) => Some(proposal.clone()),
                SignedConsensusMsg::Vote(_) => None,
            })
            .collect()
    }
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    State::new(
        TestContext::new(),
        Height::new(HEIGHT),
        ValidatorSet::new(validators.to_vec()),
        Params {
            address: my_addr,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalAndParts,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn start_height(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    validators: &[Validator],
    policy: VoteExtensionPolicy,
) {
    let vs = ValidatorSet::new(validators.to_vec());
    let harness = Harness::new(live_signature());

    // A long target time keeps the finalization timer out of the way.
    let result = process!(
        input: Input::StartHeight(
            Height::new(HEIGHT),
            vs,
            false,
            Some(Duration::from_secs(3600)),
            policy
        ),
        state: state,
        metrics: metrics,
        with: effect => harness.handle(effect)
    );

    expect_ok("StartHeight", result);
}

/// Feed `inputs` through the state machine, requiring every one of them to be accepted.
///
/// Swallowing the result here would let a test pass while consensus errored on the way.
fn drive(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    harness: &Harness,
    inputs: Vec<Input<TestContext>>,
) {
    for input in inputs {
        let label = format!("{input:?}");
        let result = process!(
            input: input,
            state: state,
            metrics: metrics,
            with: effect => harness.handle(effect)
        );

        expect_ok(&label, result);
    }
}

fn expect_ok(label: &str, result: Result<(), Error<TestContext>>) {
    if let Err(e) = result {
        panic!("consensus rejected {label}: {e}");
    }
}

fn prevote(round: u32, value: NilOrVal<ValueId>, addr: Address) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(HEIGHT), Round::new(round), value, addr),
        live_signature(),
    )
}

fn precommit(round: u32, value: NilOrVal<ValueId>, addr: Address) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(Height::new(HEIGHT), Round::new(round), value, addr),
        live_signature(),
    )
}

fn proposal(
    round: u32,
    value: &Value,
    pol_round: Round,
    addr: Address,
) -> SignedProposal<TestContext> {
    SignedProposal::new(
        Proposal::new(
            Height::new(HEIGHT),
            Round::new(round),
            value.clone(),
            pol_round,
            addr,
        ),
        live_signature(),
    )
}

/// Replay `wal` against a fresh state, with the index installed as the engine installs it.
///
/// Returns the harness so the caller can inspect what was published and how often the signer was
/// called, plus the result of the last input — replay stops at the first rejected entry, which is
/// what the engine does before halting the node.
fn replay(
    validators: &[Validator],
    my_addr: Address,
    wal: &[Input<TestContext>],
) -> (Harness, Result<(), Error<TestContext>>) {
    replay_with(
        validators,
        my_addr,
        wal,
        VoteExtensionPolicy::Disabled,
        Harness::new(replay_signature()),
    )
}

fn replay_with(
    validators: &[Validator],
    my_addr: Address,
    wal: &[Input<TestContext>],
    policy: VoteExtensionPolicy,
    harness: Harness,
) -> (Harness, Result<(), Error<TestContext>>) {
    let metrics = Metrics::new();
    let mut state = make_state(validators, my_addr);
    start_height(&mut state, &metrics, validators, policy);

    state.index_wal_entries(wal);

    let mut result = Ok(());
    for input in wal {
        result = process!(
            input: input.clone(),
            state: &mut state,
            metrics: &metrics,
            with: effect => harness.handle(effect)
        );

        if result.is_err() {
            break;
        }
    }

    if result.is_ok() {
        state.reset_entries_index();

        // The index must not outlive the replay: what it held no longer resolves. Only our own
        // messages were ever indexed, so only those make this a real check.
        for input in wal {
            match input {
                Input::Vote(vote) if *vote.validator_address() == my_addr => {
                    assert!(state.replay_index.recorded_vote(&vote.message).is_none())
                }
                Input::Proposal(proposal) if *proposal.validator_address() == my_addr => {
                    assert!(state
                        .replay_index
                        .recorded_proposal(&proposal.message)
                        .is_none())
                }
                _ => (),
            }
        }
    }

    (harness, result)
}

/// Drive a node that is not the round-0 proposer up to its nil prevote, and return what it
/// recorded. The log is `[propose timeout, our nil prevote]` — the trigger first, then the
/// message it produced.
fn capture_nil_prevote(validators: &[Validator], my_addr: Address) -> Vec<Input<TestContext>> {
    let metrics = Metrics::new();
    let mut state = make_state(validators, my_addr);
    start_height(
        &mut state,
        &metrics,
        validators,
        VoteExtensionPolicy::Disabled,
    );

    let harness = Harness::new(live_signature());
    drive(
        &mut state,
        &metrics,
        &harness,
        vec![Input::TimeoutElapsed(Timeout::propose(Round::new(0)))],
    );

    let wal = harness.wal.into_inner();
    assert!(
        matches!(wal.as_slice(), [Input::TimeoutElapsed(_), Input::Vote(_)]),
        "expected the log to hold the timeout and then our prevote, got {wal:?}"
    );

    wal
}

/// Drive the round-1 proposer through a round-0 polka and a round-0 precommit timeout, so that
/// on entering round 1 it re-proposes its valid value without asking the application. Returns
/// what it recorded.
fn capture_reproposal(
    validators: &[Validator],
    my_addr: Address,
    value: &Value,
) -> Vec<Input<TestContext>> {
    capture_reproposal_with(
        validators,
        my_addr,
        value,
        VoteExtensionPolicy::Disabled,
        Harness::new(live_signature()),
    )
}

fn capture_reproposal_with(
    validators: &[Validator],
    my_addr: Address,
    value: &Value,
    policy: VoteExtensionPolicy,
    harness: Harness,
) -> Vec<Input<TestContext>> {
    let round0_proposer = validators[0].address;
    let others: Vec<Address> = validators
        .iter()
        .map(|v| v.address)
        .filter(|addr| *addr != my_addr)
        .collect();

    let metrics = Metrics::new();
    let mut state = make_state(validators, my_addr);
    start_height(&mut state, &metrics, validators, policy);

    drive(
        &mut state,
        &metrics,
        &harness,
        vec![
            // Round 0: the proposer's value arrives, we prevote for it.
            Input::Proposal(proposal(0, value, Round::Nil, round0_proposer)),
            Input::ProposedValue(
                ProposedValue {
                    height: Height::new(HEIGHT),
                    round: Round::new(0),
                    valid_round: Round::Nil,
                    proposer: round0_proposer,
                    value: value.clone(),
                    validity: Validity::Valid,
                },
                ValueOrigin::Consensus,
            ),
            // Two more prevotes make the polka that sets our valid value, and we precommit.
            Input::Vote(prevote(0, NilOrVal::Val(value.id()), others[0])),
            Input::Vote(prevote(0, NilOrVal::Val(value.id()), others[1])),
            // Two nil precommits are enough for "some precommit quorum" without deciding
            // anything, which arms the round-0 precommit timer.
            Input::Vote(precommit(0, NilOrVal::Nil, others[0])),
            Input::Vote(precommit(0, NilOrVal::Nil, others[1])),
            // Round 0 expires; we enter round 1 as its proposer and re-propose the valid value.
            Input::TimeoutElapsed(Timeout::precommit(Round::new(0))),
        ],
    );

    let our_reproposal = harness
        .published_proposals()
        .into_iter()
        .find(|p| p.round() == Round::new(1) && *p.validator_address() == my_addr);
    assert!(
        our_reproposal.is_some(),
        "expected a round-1 re-proposal of our own; published {:?}",
        harness.published_proposals()
    );

    let wal = harness.wal.into_inner();
    assert!(
        wal.iter().any(|entry| matches!(
            entry,
            Input::Proposal(p) if p.round() == Round::new(1) && *p.validator_address() == my_addr
        )),
        "expected our round-1 re-proposal in the log, got {wal:?}"
    );

    wal
}

/// Drive a node to a non-nil precommit carrying a vote extension, then hand it a round
/// certificate that aggregates that very precommit. Returns what it recorded.
///
/// `RoundSignature` has no extension field, so a rebuilt non-nil precommit never carries one.
/// Under `Required`, that reconstruction is dropped before WAL append — matching `on_vote` —
/// so the log keeps only the extended copy we cast. When our precommit is already in the
/// keeper, the certificate path also skips re-WAL-appending it. Nil precommits from the
/// certificate are still appended.
fn capture_precommit_echoed_by_a_round_certificate(
    validators: &[Validator],
    my_addr: Address,
    value: &Value,
) -> Vec<Input<TestContext>> {
    let round0_proposer = validators[0].address;
    let others: Vec<Address> = validators
        .iter()
        .map(|v| v.address)
        .filter(|addr| *addr != my_addr)
        .collect();

    let metrics = Metrics::new();
    let mut state = make_state(validators, my_addr);
    start_height(
        &mut state,
        &metrics,
        validators,
        VoteExtensionPolicy::Required,
    );

    let harness = Harness::new(live_signature()).with_extension(b"extension-from-the-live-run");
    drive(
        &mut state,
        &metrics,
        &harness,
        vec![
            Input::Proposal(proposal(0, value, Round::Nil, round0_proposer)),
            Input::ProposedValue(
                ProposedValue {
                    height: Height::new(HEIGHT),
                    round: Round::new(0),
                    valid_round: Round::Nil,
                    proposer: round0_proposer,
                    value: value.clone(),
                    validity: Validity::Valid,
                },
                ValueOrigin::Consensus,
            ),
            Input::Vote(prevote(0, NilOrVal::Val(value.id()), others[0])),
            Input::Vote(prevote(0, NilOrVal::Val(value.id()), others[1])),
        ],
    );

    let our_precommit = harness
        .published_votes()
        .into_iter()
        .find(|v| v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr)
        .expect("the polka should have made us precommit the value");
    assert!(
        our_precommit.extension().is_some(),
        "the precommit we cast should carry the extension the application supplied"
    );

    // A round certificate covering round 0 comes back to us with our own precommit in it.
    drive(
        &mut state,
        &metrics,
        &harness,
        vec![Input::RoundCertificate(RoundCertificate::new_from_votes(
            Height::new(HEIGHT),
            Round::new(0),
            RoundCertificateType::Precommit,
            vec![
                our_precommit.clone(),
                precommit(0, NilOrVal::Nil, others[0]),
                precommit(0, NilOrVal::Nil, others[1]),
            ],
        ))],
    );

    let wal = harness.wal.into_inner();

    let ours: Vec<&SignedVote<TestContext>> = wal
        .iter()
        .filter_map(|entry| match entry {
            Input::Vote(v)
                if v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr =>
            {
                Some(v)
            }
            _ => None,
        })
        .collect();

    // Under Required the certificate's extensionless rebuild of our precommit is dropped; when
    // our precommit is already in the keeper, the certificate must not WAL-append a second copy.
    assert_eq!(
        ours.len(),
        1,
        "expected our precommit to be recorded once — the extended cast, not a \
         certificate reconstruction — got {ours:?}"
    );
    assert!(
        ours[0].extension().is_some(),
        "the single record must be the extended copy we cast, got {ours:?}"
    );

    wal
}

fn four_validators() -> Vec<Validator> {
    make_validators([1, 1, 1, 1])
        .into_iter()
        .map(|(v, _)| v)
        .collect()
}

#[test]
fn replayed_vote_is_republished_from_the_record_without_signing() {
    let validators = four_validators();
    // Round-0 proposer is validators[0], so validators[1] prevotes nil on the propose timeout.
    let my_addr = validators[1].address;

    let wal = capture_nil_prevote(&validators, my_addr);
    let Some(Input::Vote(recorded)) = wal.last().cloned() else {
        unreachable!("capture_nil_prevote asserts the shape of the log")
    };

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_votes.get(),
        0,
        "a vote already in the log must not be signed again"
    );
    assert_eq!(
        harness.published_votes(),
        vec![recorded],
        "replay must republish the recorded vote, signature included"
    );
}

#[test]
fn vote_at_the_frontier_is_signed_and_published() {
    let validators = four_validators();
    let my_addr = validators[1].address;

    // The crash landed between the timeout and the vote it triggered: the log holds the trigger
    // only. This is live behaviour, and withholding the vote here would leave the node unable to
    // arm the timers that carry it out of the round.
    let mut wal = capture_nil_prevote(&validators, my_addr);
    wal.pop();

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_votes.get(),
        1,
        "with nothing recorded for it, the vote must be signed"
    );

    let published = harness.published_votes();
    assert_eq!(published.len(), 1, "expected one published vote");
    assert_eq!(published[0].signature, replay_signature());
    assert_eq!(published[0].vote_type(), VoteType::Prevote);
    assert_eq!(published[0].round(), Round::new(0));
    assert_eq!(*published[0].value(), NilOrVal::Nil);
}

#[test]
fn diverging_vote_fails_the_replay_and_publishes_nothing() {
    let validators = four_validators();
    let my_addr = validators[1].address;

    // Same log, except the record says we prevoted for a value where re-derivation prevotes nil.
    let mut wal = capture_nil_prevote(&validators, my_addr);
    *wal.last_mut().unwrap() =
        Input::Vote(prevote(0, NilOrVal::Val(ValueId::new(0x1234)), my_addr));

    let (harness, result) = replay(&validators, my_addr, &wal);

    match result {
        Err(Error::ReplayDivergence(kind, height, round)) => {
            assert_eq!(kind, RecordKind::Prevote);
            assert_eq!(height, Height::new(HEIGHT));
            assert_eq!(round, Round::new(0));
        }
        other => panic!("expected a replay divergence, got {other:?}"),
    }

    assert_eq!(
        harness.signed_votes.get(),
        0,
        "a divergent vote must not be signed"
    );
    assert!(
        harness.published_votes().is_empty(),
        "a divergent vote must not reach peers"
    );
}

#[test]
fn replayed_proposal_is_republished_from_the_record_without_signing() {
    let validators = four_validators();
    // Round-1 proposer is validators[1]; it re-proposes its round-0 valid value.
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    let wal = capture_reproposal(&validators, my_addr, &value);
    let recorded = wal
        .iter()
        .find_map(|entry| match entry {
            Input::Proposal(p)
                if p.round() == Round::new(1) && *p.validator_address() == my_addr =>
            {
                Some(p.clone())
            }
            _ => None,
        })
        .expect("capture_reproposal asserts the re-proposal is in the log");

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_proposals.get(),
        0,
        "a proposal already in the log must not be signed again"
    );
    assert!(
        harness.published_proposals().contains(&recorded),
        "replay must republish the recorded proposal, signature included; published {:?}",
        harness.published_proposals()
    );
}

#[test]
fn a_double_signed_log_still_replays_from_the_record_rather_than_signing_again() {
    let validators = four_validators();
    let my_addr = validators[1].address;

    // A log that already holds two conflicting votes of ours for one key: we double-signed
    // before the crash. Nothing here can undo that, but replay must not compound it — dropping
    // both records would send us back to the signer for a *third* message under the same key,
    // which is the outcome the log exists to prevent.
    let mut wal = capture_nil_prevote(&validators, my_addr);
    let Some(Input::Vote(recorded)) = wal.last().cloned() else {
        unreachable!("capture_nil_prevote asserts the shape of the log")
    };
    wal.push(Input::Vote(prevote(
        0,
        NilOrVal::Val(ValueId::new(0x1234)),
        my_addr,
    )));

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_votes.get(),
        0,
        "a conflicting second record must not send replay back to the signer"
    );
    assert_eq!(
        harness.published_votes(),
        vec![recorded],
        "the vote recorded first must be the one republished"
    );
}

#[test]
fn diverging_proposal_fails_the_replay() {
    let validators = four_validators();
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    // Same log, except the record says we proposed a different value in round 1.
    let mut wal = capture_reproposal(&validators, my_addr, &value);
    let recorded = wal
        .iter_mut()
        .find(|entry| {
            matches!(entry, Input::Proposal(p) if p.round() == Round::new(1) && *p.validator_address() == my_addr)
        })
        .expect("capture_reproposal asserts the re-proposal is in the log");
    *recorded = Input::Proposal(proposal(1, &Value::new(0xdead), Round::new(0), my_addr));

    let (harness, result) = replay(&validators, my_addr, &wal);

    match result {
        Err(Error::ReplayDivergence(kind, height, round)) => {
            assert_eq!(kind, RecordKind::Proposal);
            assert_eq!(height, Height::new(HEIGHT));
            assert_eq!(round, Round::new(1));
        }
        other => panic!("expected a replay divergence, got {other:?}"),
    }

    assert_eq!(
        harness.signed_proposals.get(),
        0,
        "a divergent proposal must not be signed"
    );
}

#[test]
fn diverging_precommit_fails_the_replay_and_publishes_nothing() {
    let validators = four_validators();
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    // The prevote case is covered above, but precommits are the slashable ones, so the fatal path
    // is pinned for both vote types rather than assumed to generalise.
    let mut wal = capture_reproposal(&validators, my_addr, &value);
    let recorded = wal
        .iter_mut()
        .find(|entry| {
            matches!(entry, Input::Vote(v) if v.vote_type() == VoteType::Precommit
                && *v.validator_address() == my_addr)
        })
        .expect("the live run precommits the value it prevoted for");
    *recorded = Input::Vote(precommit(0, NilOrVal::Val(ValueId::new(0xdead)), my_addr));

    let (harness, result) = replay(&validators, my_addr, &wal);

    match result {
        Err(Error::ReplayDivergence(kind, height, round)) => {
            assert_eq!(kind, RecordKind::Precommit);
            assert_eq!(height, Height::new(HEIGHT));
            assert_eq!(round, Round::new(0));
        }
        other => panic!("expected a replay divergence, got {other:?}"),
    }

    assert_eq!(
        harness.signed_votes.get(),
        0,
        "a divergent precommit must not be signed"
    );
    assert!(
        !harness
            .published_votes()
            .iter()
            .any(|v| v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr),
        "a divergent precommit must not reach peers"
    );
}

#[test]
fn proposal_at_the_frontier_is_signed_and_published() {
    let validators = four_validators();
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    // The crash landed between deriving the round-1 re-proposal and recording it, so the log
    // stops short of it. As with the vote frontier, this is live behaviour and must keep working.
    let mut wal = capture_reproposal(&validators, my_addr, &value);
    let record = wal
        .iter()
        .position(|entry| {
            matches!(entry, Input::Proposal(p) if p.round() == Round::new(1)
                && *p.validator_address() == my_addr)
        })
        .expect("capture_reproposal asserts the re-proposal is in the log");
    wal.truncate(record);

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_proposals.get(),
        1,
        "with nothing recorded for it, the re-proposal must be signed"
    );

    let ours: Vec<_> = harness
        .published_proposals()
        .into_iter()
        .filter(|p| p.round() == Round::new(1) && *p.validator_address() == my_addr)
        .collect();
    assert_eq!(ours.len(), 1, "expected one round-1 proposal of our own");
    assert_eq!(ours[0].signature, replay_signature());
    assert_eq!(*ours[0].value(), value);
}

#[test]
fn vote_whose_extension_alone_differs_is_republished_from_the_record() {
    let validators = four_validators();
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    // Vote extensions come from the application, which may build them from data consensus knows
    // nothing about and sign them non-deterministically. A precommit that agrees on everything
    // the algorithm acts upon but carries a different extension is therefore not evidence of a
    // state-machine bug, and halting on it would be a self-inflicted outage.
    let wal = capture_reproposal_with(
        &validators,
        my_addr,
        &value,
        VoteExtensionPolicy::Required,
        Harness::new(live_signature()).with_extension(b"extension-from-the-live-run"),
    );

    let recorded = wal
        .iter()
        .find_map(|entry| match entry {
            Input::Vote(v)
                if v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr =>
            {
                Some(v.clone())
            }
            _ => None,
        })
        .expect("the live run precommits the value it prevoted for");
    assert!(
        recorded.extension().is_some(),
        "the recorded precommit should carry the live run's extension"
    );

    let (harness, result) = replay_with(
        &validators,
        my_addr,
        &wal,
        VoteExtensionPolicy::Required,
        Harness::new(replay_signature()).with_extension(b"extension-from-the-replay"),
    );
    expect_ok("replay", result);

    assert!(
        harness.extended_votes.get() > 0,
        "the replay must have re-derived the precommit and asked the application for a fresh \
         extension, otherwise this test proves nothing"
    );
    assert_eq!(
        harness.signed_votes.get(),
        0,
        "an extension-only difference must not send the vote back to the signer"
    );

    let ours: Vec<_> = harness
        .published_votes()
        .into_iter()
        .filter(|v| v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr)
        .collect();
    assert_eq!(
        ours,
        vec![recorded],
        "the recorded precommit must go out whole: its extension and the signature over it have \
         to stay consistent"
    );
}

#[test]
fn votes_from_other_validators_are_not_indexed() {
    let validators = four_validators();
    let my_addr = validators[1].address;

    // A peer's vote sits at the same (kind, height, round) as the one we are about to derive.
    // Indexing it would make us republish someone else's vote in place of our own.
    let mut wal = capture_nil_prevote(&validators, my_addr);
    wal.pop();
    wal.push(Input::Vote(prevote(
        0,
        NilOrVal::Val(ValueId::new(0x1234)),
        validators[2].address,
    )));

    let (harness, result) = replay(&validators, my_addr, &wal);
    expect_ok("replay", result);

    assert_eq!(
        harness.signed_votes.get(),
        1,
        "only our own records suppress signing"
    );

    let ours: Vec<_> = harness
        .published_votes()
        .into_iter()
        .filter(|v| *v.validator_address() == my_addr)
        .collect();
    assert_eq!(ours.len(), 1);
    assert_eq!(*ours[0].value(), NilOrVal::Nil);
    assert_eq!(ours[0].signature, replay_signature());
}

#[test]
fn a_precommit_echoed_back_by_a_round_certificate_still_verifies() {
    let validators = four_validators();
    let my_addr = validators[1].address;
    let value = Value::new(0x9e57);

    let wal = capture_precommit_echoed_by_a_round_certificate(&validators, my_addr, &value);
    let cast = wal
        .iter()
        .find_map(|entry| match entry {
            Input::Vote(v)
                if v.vote_type() == VoteType::Precommit
                    && *v.validator_address() == my_addr
                    && v.extension().is_some() =>
            {
                Some(v.clone())
            }
            _ => None,
        })
        .expect("the capture asserts the extended copy is in the log");

    let (harness, result) = replay_with(
        &validators,
        my_addr,
        &wal,
        VoteExtensionPolicy::Required,
        Harness::new(replay_signature()).with_extension(b"extension-from-the-replay"),
    );
    expect_ok("replay", result);

    // The certificate rebuild was never WAL-appended under Required, so the index sees only
    // the extended copy we cast. Replay must still republish that record without re-signing.
    assert_eq!(
        harness.signed_votes.get(),
        0,
        "the recorded extended precommit must suppress signing on replay"
    );

    let ours: Vec<_> = harness
        .published_votes()
        .into_iter()
        .filter(|v| v.vote_type() == VoteType::Precommit && *v.validator_address() == my_addr)
        .collect();
    assert_eq!(
        ours,
        vec![cast],
        "the precommit we cast must be republished, extension included"
    );
}
