/// A round certificate whose votes are already in the vote keeper must skip
/// verification and WAL appends. The driver's single stored slot is not used
/// as the already-seen check.
use std::cell::Cell;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, Resumable, Resume, State,
};
use malachitebft_core_types::{
    Context, NilOrVal, Round, RoundCertificate, RoundCertificateType, SignedVote, ValuePayload,
};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Height, Signature, TestContext, Validator, ValidatorSet, ValueId,
};

#[derive(Default)]
struct EffectCounts {
    verify_round_certificate: Cell<u32>,
    wal_append_vote: Cell<u32>,
}

impl EffectCounts {
    fn handle(&self, effect: Effect<TestContext>) -> Result<Resume<TestContext>, ()> {
        use Effect::*;

        Ok(match effect {
            VerifyRoundCertificate(_, _, _, r) => {
                self.verify_round_certificate
                    .set(self.verify_round_certificate.get() + 1);
                r.resume_with(Ok(()))
            }
            WalAppend(_, Input::Vote(_), r) => {
                self.wal_append_vote.set(self.wal_append_vote.get() + 1);
                r.resume_with(())
            }
            WalAppend(_, _, r) => r.resume_with(()),
            VerifySignature(_, _, r) => r.resume_with(true),
            VerifyCommitCertificate(_, _, _, r) => r.resume_with(Ok(())),
            SignVote(vote, r) => r.resume_with(SignedVote::new(vote, Signature::test())),
            _ => Resume::Continue,
        })
    }
}

fn make_state(validators: &[Validator], address: Address) -> State<TestContext> {
    State::new(
        TestContext::new(),
        Height::new(1),
        ValidatorSet::new(validators.to_vec()),
        Params {
            address,
            threshold_params: Default::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        },
        1000,
        1000,
    )
}

fn drive(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    counts: &EffectCounts,
    input: Input<TestContext>,
) {
    let result: Result<(), Error<TestContext>> = process!(
        input: input,
        state: state,
        metrics: metrics,
        with: effect => counts.handle(effect)
    );
    result.expect("consensus input should be processed");
}

fn precommit_value(address: Address, round: Round) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_precommit(
        Height::new(1),
        round,
        NilOrVal::Val(ValueId::new(1)),
        address,
    );
    SignedVote::new(vote, Signature::test())
}

fn prevote_value(address: Address, round: Round) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_prevote(
        Height::new(1),
        round,
        NilOrVal::Val(ValueId::new(1)),
        address,
    );
    SignedVote::new(vote, Signature::test())
}

fn started_state() -> (Vec<Validator>, State<TestContext>, Metrics, EffectCounts) {
    let validators: Vec<_> = make_validators([25, 25, 25, 25])
        .into_iter()
        .map(|(validator, _)| validator)
        .collect();
    let mut state = make_state(&validators, validators[0].address);
    let metrics = Metrics::new();
    let counts = EffectCounts::default();

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::StartHeight(
            Height::new(1),
            ValidatorSet::new(validators.clone()),
            false,
            None,
            Default::default(),
        ),
    );

    (validators, state, metrics, counts)
}

fn precommit_value_certificate(
    validators: &[Validator],
    round: Round,
    members: &[usize],
) -> RoundCertificate<TestContext> {
    RoundCertificate::new_from_votes(
        Height::new(1),
        round,
        RoundCertificateType::Precommit,
        members
            .iter()
            .map(|&i| precommit_value(validators[i].address, round))
            .collect(),
    )
}

#[test]
fn resent_precommit_value_certificate_skips_verify_and_wal() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(0);
    let certificate = precommit_value_certificate(&validators, round, &[0, 1, 2]);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate.clone()),
    );

    // A Precommit-for-value quorum at the current round still leaves the
    // driver's single slot empty; skip is by vote-keeper absorption, not the slot.
    let slot_matches_height_round_type = state.round_certificate().is_some_and(|existing| {
        existing.certificate.height == certificate.height
            && existing.certificate.round == certificate.round
            && existing.certificate.cert_type == certificate.cert_type
    });
    assert!(
        !slot_matches_height_round_type,
        "driver slot must not make the old height/round/type check skip this resend"
    );

    assert_eq!(counts.verify_round_certificate.get(), 1);
    let wal_after_first = counts.wal_append_vote.get();
    assert!(
        wal_after_first > 0,
        "first delivery should persist the certificate votes"
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate),
    );

    assert_eq!(counts.verify_round_certificate.get(), 1);
    assert_eq!(counts.wal_append_vote.get(), wal_after_first);
}

#[test]
fn absorbed_votes_skip_round_certificate_verify_and_wal() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(0);

    for &i in &[0, 1, 2] {
        drive(
            &mut state,
            &metrics,
            &counts,
            Input::Vote(precommit_value(validators[i].address, round)),
        );
    }

    assert_eq!(counts.verify_round_certificate.get(), 0);
    let wal_after_votes = counts.wal_append_vote.get();
    assert!(
        wal_after_votes > 0,
        "gossip votes should persist to the WAL"
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(precommit_value_certificate(&validators, round, &[0, 1, 2])),
    );

    assert_eq!(counts.verify_round_certificate.get(), 0);
    assert_eq!(counts.wal_append_vote.get(), wal_after_votes);
}

#[test]
fn different_precommit_value_certificate_is_verified() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(0);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(precommit_value_certificate(&validators, round, &[0, 1, 2])),
    );
    let wal_after_first = counts.wal_append_vote.get();
    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(precommit_value_certificate(&validators, round, &[1, 2, 3])),
    );

    assert_eq!(counts.verify_round_certificate.get(), 2);
    assert_eq!(
        counts.wal_append_vote.get(),
        wal_after_first + 1,
        "only the vote not already in the keeper should be WAL-appended"
    );
}

fn precommit_for(
    address: Address,
    round: Round,
    value: NilOrVal<ValueId>,
) -> SignedVote<TestContext> {
    let vote = TestContext::new().new_precommit(Height::new(1), round, value, address);
    SignedVote::new(vote, Signature::test())
}

/// A certificate vote that conflicts with one we already hold is recorded as
/// evidence on first delivery. A resend of that certificate must not pay for
/// verification or WAL again: the stored vote still differs, so `has_vote` is
/// false, but the pair is already in the evidence map.
#[test]
fn resent_certificate_with_recorded_equivocation_skips_verify_and_wal() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(0);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::Vote(precommit_for(
            validators[0].address,
            round,
            NilOrVal::Val(ValueId::new(1)),
        )),
    );
    let wal_after_vote = counts.wal_append_vote.get();

    let certificate = RoundCertificate::new_from_votes(
        Height::new(1),
        round,
        RoundCertificateType::Precommit,
        vec![
            precommit_for(validators[0].address, round, NilOrVal::Val(ValueId::new(2))),
            precommit_for(validators[1].address, round, NilOrVal::Val(ValueId::new(1))),
            precommit_for(validators[2].address, round, NilOrVal::Val(ValueId::new(1))),
        ],
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate.clone()),
    );

    assert_eq!(counts.verify_round_certificate.get(), 1);
    let wal_after_first = counts.wal_append_vote.get();
    assert!(
        wal_after_first > wal_after_vote,
        "first delivery should persist the new certificate votes"
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate),
    );

    assert_eq!(
        counts.verify_round_certificate.get(),
        1,
        "a resend whose only novel vote is already recorded as equivocation must not re-verify"
    );
    assert_eq!(counts.wal_append_vote.get(), wal_after_first);
}

fn skip_certificate(
    validators: &[Validator],
    round: Round,
    members: &[usize],
) -> RoundCertificate<TestContext> {
    RoundCertificate::new_from_votes(
        Height::new(1),
        round,
        RoundCertificateType::Skip,
        members
            .iter()
            .map(|&i| prevote_value(validators[i].address, round))
            .collect(),
    )
}

#[test]
fn resent_skip_round_certificate_skips_verify_and_wal() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(1);
    let certificate = skip_certificate(&validators, round, &[1, 2]);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate.clone()),
    );

    assert_eq!(counts.verify_round_certificate.get(), 1);
    let wal_after_first = counts.wal_append_vote.get();
    assert!(
        wal_after_first > 0,
        "first delivery should persist the certificate votes"
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate),
    );

    assert_eq!(counts.verify_round_certificate.get(), 1);
    assert_eq!(counts.wal_append_vote.get(), wal_after_first);
}

#[test]
fn same_height_restart_allows_the_same_certificate_again() {
    let (validators, mut state, metrics, counts) = started_state();
    let round = Round::new(0);
    let certificate = precommit_value_certificate(&validators, round, &[0, 1, 2]);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate.clone()),
    );
    assert_eq!(counts.verify_round_certificate.get(), 1);

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::StartHeight(
            Height::new(1),
            ValidatorSet::new(validators.clone()),
            true,
            None,
            Default::default(),
        ),
    );

    drive(
        &mut state,
        &metrics,
        &counts,
        Input::RoundCertificate(certificate),
    );

    assert_eq!(counts.verify_round_certificate.get(), 2);
}
