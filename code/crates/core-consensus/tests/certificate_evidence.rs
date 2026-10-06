//! Equivocation evidence carried by certificates the liveness handler does not process.
//!
//! A certificate is identified by the polka it witnesses or by the round it justifies, never by
//! its signer set, so a certificate that is skipped can still hold the only copy of a vote that
//! conflicts with one already recorded. Each test seeds a conflicting vote, drives the handler
//! into one of its skip paths, and asserts the evidence is recorded while the WAL append and the
//! signature verifications of the non-conflicting signers are still skipped.

use std::vec::Vec;

use arc_malachitebft_core_consensus::{
    process, ConsensusMsg, Effect, Error, Input, Params, Resumable, Resume, State, ValuePayload,
};
use malachitebft_core_types::{
    NilOrVal, PolkaCertificate, Round, RoundCertificate, RoundCertificateType, SignedVote, Timeout,
};
// Brings the `Vote` trait accessors into scope without colliding with the concrete `Vote` type
// re-exported from `malachitebft_test`.
use malachitebft_core_types::Vote as _;
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Signature, TestContext, Validator, ValidatorSet, Value, ValueId, Vote,
};

/// Voting powers totalling 7: any pair including `v2` reaches 5, which clears both the 2/3
/// quorum (> 4.66) and the f+1 honest threshold (3). `v1 + v3` reaches only 4 and does not.
const VOTING_POWER: [u64; 3] = [2, 3, 2];

#[derive(Default)]
struct Captured {
    wal: Vec<Input<TestContext>>,
    verified_vote_signers: Vec<Address>,
}

fn handle_effect(
    effect: Effect<TestContext>,
    cap: &mut Captured,
) -> Result<Resume<TestContext>, ()> {
    use Effect::*;
    Ok(match effect {
        VerifySignature(msg, _, r) => {
            if let ConsensusMsg::Vote(vote) = &msg.message {
                cap.verified_vote_signers.push(*vote.validator_address());
            }
            r.resume_with(true)
        }
        VerifyPolkaCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyRoundCertificate(_, _, _, r) => r.resume_with(Ok(())),
        VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
        SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
        WalAppend(_, entry, r) => {
            cap.wal.push(entry);
            r.resume_with(())
        }
        ExtendVote(_, _, _, _, r) => r.resume_with(None),
        VerifyVoteExtension(_, _, _, _, _, _, r) => r.resume_with(Ok(())),
        _ => Resume::Continue,
    })
}

fn drive(
    state: &mut State<TestContext>,
    inputs: Vec<Input<TestContext>>,
    cap: &mut Captured,
    metrics: &Metrics,
) {
    for input in inputs {
        let _: Result<(), Error<TestContext>> = process!(
            input: input,
            state: state,
            metrics: metrics,
            with: e => handle_effect(e, cap)
        );
    }
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    State::new(
        TestContext::new(),
        Height::new(1),
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

fn prevote(addr: Address, round: u32, value_id: NilOrVal<ValueId>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_prevote(Height::new(1), Round::new(round), value_id, addr),
        Signature::test(),
    )
}

fn precommit(addr: Address, round: u32, value_id: NilOrVal<ValueId>) -> SignedVote<TestContext> {
    SignedVote::new(
        Vote::new_precommit(Height::new(1), Round::new(round), value_id, addr),
        Signature::test(),
    )
}

#[test]
fn round_certificate_reconstructs_votes_of_either_type() {
    let value_id = Value::new(0x37a5).id();
    let expected = vec![
        prevote(Address::new([1; 20]), 0, NilOrVal::Val(value_id)),
        precommit(Address::new([2; 20]), 0, NilOrVal::Nil),
    ];
    let certificate = RoundCertificate::new_from_votes(
        Height::new(1),
        Round::new(0),
        RoundCertificateType::Precommit,
        expected.clone(),
    );

    let context = TestContext::new();
    let reconstructed: Vec<_> = certificate.votes(&context).collect();
    assert_eq!(reconstructed, expected);
}

fn polka_certificate(
    round: u32,
    value_id: ValueId,
    votes: Vec<SignedVote<TestContext>>,
) -> Input<TestContext> {
    Input::PolkaCertificate(PolkaCertificate::new(
        Height::new(1),
        Round::new(round),
        value_id,
        votes,
    ))
}

fn round_certificate(
    round: u32,
    cert_type: RoundCertificateType,
    votes: Vec<SignedVote<TestContext>>,
) -> Input<TestContext> {
    Input::RoundCertificate(RoundCertificate::new_from_votes(
        Height::new(1),
        Round::new(round),
        cert_type,
        votes,
    ))
}

fn vote_evidence_count(state: &State<TestContext>, addr: Address) -> usize {
    state
        .driver
        .votes()
        .evidence()
        .get(&addr)
        .map(|v: &Vec<_>| v.len())
        .unwrap_or(0)
}

struct Fixture {
    state: State<TestContext>,
    cap: Captured,
    metrics: Metrics,
    validators: Vec<Validator>,
}

impl Fixture {
    /// A node (`v3`) started at height 1, round 0, with the validator set installed.
    fn new() -> Self {
        let validators: Vec<_> = make_validators(VOTING_POWER)
            .into_iter()
            .map(|(v, _)| v)
            .collect();
        let me = validators[2].address;

        let mut fixture = Self {
            state: make_state(&validators, me),
            cap: Captured::default(),
            metrics: Metrics::new(),
            validators,
        };

        let vs = ValidatorSet::new(fixture.validators.clone());
        fixture.drive(vec![Input::StartHeight(
            Height::new(1),
            vs,
            false,
            None,
            Default::default(),
        )]);

        fixture
    }

    fn v1(&self) -> Address {
        self.validators[0].address
    }

    fn v2(&self) -> Address {
        self.validators[1].address
    }

    fn v3(&self) -> Address {
        self.validators[2].address
    }

    fn drive(&mut self, inputs: Vec<Input<TestContext>>) {
        drive(&mut self.state, inputs, &mut self.cap, &self.metrics);
    }

    fn wal_len(&self) -> usize {
        self.cap.wal.len()
    }

    /// The signers whose signatures were verified since the last call.
    fn take_verified_signers(&mut self) -> Vec<Address> {
        std::mem::take(&mut self.cap.verified_vote_signers)
    }
}

#[test]
fn skipped_polka_certificate_records_equivocation_for_a_conflicting_signer() {
    let mut f = Fixture::new();
    let (v1, v2, v3) = (f.v1(), f.v2(), f.v3());
    let value_id = Value::new(0x37a5).id();

    // v1 prevotes nil, then v2 and v3 carry the value to a prevote quorum, so a polka
    // certificate for the value is stored without v1.
    f.drive(vec![
        Input::Vote(prevote(v1, 0, NilOrVal::Nil)),
        polka_certificate(
            0,
            value_id,
            vec![
                prevote(v2, 0, NilOrVal::Val(value_id)),
                prevote(v3, 0, NilOrVal::Val(value_id)),
            ],
        ),
    ]);
    assert!(
        f.state
            .polka_certificate(Round::new(0), &value_id)
            .is_some(),
        "the polka certificate must be stored for the skip path to be exercised"
    );
    assert_eq!(vote_evidence_count(&f.state, v1), 0);

    let wal_before = f.wal_len();
    let _ = f.take_verified_signers();

    // A second certificate for the same round and value is skipped, but it carries v1's
    // prevote for the value, which conflicts with the prevote for nil already recorded.
    f.drive(vec![polka_certificate(
        0,
        value_id,
        vec![
            prevote(v1, 0, NilOrVal::Val(value_id)),
            prevote(v2, 0, NilOrVal::Val(value_id)),
        ],
    )]);

    assert_eq!(
        vote_evidence_count(&f.state, v1),
        1,
        "the conflicting signer's equivocation should be recorded"
    );
    assert_eq!(
        f.wal_len(),
        wal_before,
        "the skipped certificate should not be appended to the WAL"
    );
    assert_eq!(
        f.take_verified_signers(),
        vec![v1],
        "only the conflicting signature should be verified"
    );

    // A further certificate carrying no conflicting signer records nothing and verifies nothing.
    f.drive(vec![polka_certificate(
        0,
        value_id,
        vec![prevote(v2, 0, NilOrVal::Val(value_id))],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(vote_evidence_count(&f.state, v2), 0);
    assert_eq!(f.wal_len(), wal_before);
    assert!(
        f.take_verified_signers().is_empty(),
        "a certificate raising no conflict should cost no signature verification"
    );
}

#[test]
fn skipped_precommit_round_certificate_from_an_older_round_records_equivocation() {
    let mut f = Fixture::new();
    let (v1, v2) = (f.v1(), f.v2());
    let value_id = Value::new(0x37a5).id();

    // A precommit certificate at round 0 records v1's precommit for nil and schedules the
    // precommit timer; the timer then moves us to round 1.
    f.drive(vec![
        round_certificate(
            0,
            RoundCertificateType::Precommit,
            vec![
                precommit(v1, 0, NilOrVal::Nil),
                precommit(v2, 0, NilOrVal::Nil),
            ],
        ),
        Input::TimeoutElapsed(Timeout::precommit(Round::new(0))),
    ]);
    assert_eq!(f.state.round(), Round::new(1));
    assert_eq!(vote_evidence_count(&f.state, v1), 0);

    let wal_before = f.wal_len();
    let _ = f.take_verified_signers();

    // A precommit certificate from round 0 no longer advances the round, but it carries v1's
    // precommit for the value, conflicting with the precommit for nil already recorded.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Val(value_id)),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(f.wal_len(), wal_before);
    assert_eq!(f.take_verified_signers(), vec![v1]);

    // A further certificate carrying no conflicting signer records nothing and verifies nothing.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Nil),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(vote_evidence_count(&f.state, v2), 0);
    assert_eq!(f.wal_len(), wal_before);
    assert!(
        f.take_verified_signers().is_empty(),
        "a certificate raising no conflict should cost no signature verification"
    );
}

#[test]
fn skipped_skip_round_certificate_from_the_current_round_records_equivocation() {
    let mut f = Fixture::new();
    let (v1, v2) = (f.v1(), f.v2());
    let value_id = Value::new(0x37a5).id();

    f.drive(vec![Input::Vote(prevote(v1, 0, NilOrVal::Nil))]);
    assert_eq!(f.state.round(), Round::new(0));

    let wal_before = f.wal_len();
    let _ = f.take_verified_signers();

    // A skip certificate for the round we are already in does not advance the round, but it
    // carries v1's prevote for the value, conflicting with the prevote for nil.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Skip,
        vec![
            prevote(v1, 0, NilOrVal::Val(value_id)),
            prevote(v2, 0, NilOrVal::Val(value_id)),
        ],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(vote_evidence_count(&f.state, v2), 0);
    assert_eq!(
        f.wal_len(),
        wal_before,
        "the skipped certificate's votes should not be appended to the WAL"
    );
    assert_eq!(f.take_verified_signers(), vec![v1]);

    // A further certificate carrying no conflicting signer records nothing and verifies nothing.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Skip,
        vec![prevote(v2, 0, NilOrVal::Val(value_id))],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(vote_evidence_count(&f.state, v2), 0);
    assert_eq!(f.wal_len(), wal_before);
    assert!(
        f.take_verified_signers().is_empty(),
        "a certificate raising no conflict should cost no signature verification"
    );
}

#[test]
fn skipped_already_known_round_certificate_records_equivocation() {
    let mut f = Fixture::new();
    let (v1, v2) = (f.v1(), f.v2());
    let value_id = Value::new(0x37a5).id();

    // A precommit certificate at round 0 is stored as the round certificate and records v1's
    // precommit for nil. We stay in round 0 until the precommit timer fires.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Nil),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);
    assert_eq!(f.state.round(), Round::new(0));
    assert!(f.state.round_certificate().is_some());
    assert_eq!(vote_evidence_count(&f.state, v1), 0);

    let wal_before = f.wal_len();
    let _ = f.take_verified_signers();

    // A certificate with the same height, round and type is already known, but it carries v1's
    // precommit for the value, conflicting with the precommit for nil already recorded.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Val(value_id)),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(
        f.wal_len(),
        wal_before,
        "the skipped certificate's votes should not be appended to the WAL"
    );
    assert_eq!(f.take_verified_signers(), vec![v1]);

    // A certificate with the same identity and no conflicting signer records nothing.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Nil),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(vote_evidence_count(&f.state, v2), 0);
    assert_eq!(f.wal_len(), wal_before);
    assert!(
        f.take_verified_signers().is_empty(),
        "a certificate raising no conflict should cost no signature verification"
    );
}

/// A skipped certificate is re-delivered on every gossip round, and its conflicting vote is
/// never stored as a vote, so the conflict is re-detected each time. The recorded evidence pair
/// is what stops the signature being verified again.
#[test]
fn a_re_delivered_certificate_does_not_verify_an_already_recorded_equivocation_again() {
    let mut f = Fixture::new();
    let (v1, v2) = (f.v1(), f.v2());
    let value_id = Value::new(0x37a5).id();

    let conflicting = || {
        round_certificate(
            0,
            RoundCertificateType::Precommit,
            vec![
                precommit(v1, 0, NilOrVal::Val(value_id)),
                precommit(v2, 0, NilOrVal::Nil),
            ],
        )
    };

    // A precommit certificate at round 0 is stored and records v1's precommit for nil.
    f.drive(vec![round_certificate(
        0,
        RoundCertificateType::Precommit,
        vec![
            precommit(v1, 0, NilOrVal::Nil),
            precommit(v2, 0, NilOrVal::Nil),
        ],
    )]);

    let wal_before = f.wal_len();
    let _ = f.take_verified_signers();

    f.drive(vec![conflicting()]);
    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(f.take_verified_signers(), vec![v1]);

    // Two further deliveries of the same certificate neither grow the evidence nor re-verify.
    f.drive(vec![conflicting(), conflicting()]);

    assert_eq!(vote_evidence_count(&f.state, v1), 1);
    assert_eq!(f.wal_len(), wal_before);
    assert!(
        f.take_verified_signers().is_empty(),
        "an equivocation already stored as evidence should not be verified again"
    );
}
