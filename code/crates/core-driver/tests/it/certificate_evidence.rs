//! Tests that equivocating signatures delivered inside certificates are
//! recorded as evidence by the vote keeper.

use std::sync::Arc;

use malachitebft_core_types::{
    CommitCertificate, CommitSignature, ExtendedCommitCertificate, NilOrVal, PolkaCertificate,
    PolkaSignature, Round, SignedVote, Vote as _, VoteExtensions,
};
use malachitebft_test::proposer_selector::{FixedProposer, ProposerSelector};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value, Vote};

use arc_malachitebft_core_driver::{Driver, Input};

fn sign_prevote(
    sk: &PrivateKey,
    height: Height,
    round: Round,
    value: &Value,
    address: Address,
) -> PolkaSignature<TestContext> {
    let vote = Vote::new_prevote(height, round, NilOrVal::Val(value.id()), address);
    let signature = sk.sign(&vote.to_sign_bytes());
    PolkaSignature::new(address, signature)
}

fn sign_precommit(
    sk: &PrivateKey,
    height: Height,
    round: Round,
    value: &Value,
    address: Address,
) -> CommitSignature<TestContext> {
    let vote = Vote::new_precommit(height, round, NilOrVal::Val(value.id()), address);
    let signature = sk.sign(&vote.to_sign_bytes());
    CommitSignature::new(address, signature)
}

fn new_driver(my_addr: Address, validator_set: ValidatorSet) -> Driver<TestContext> {
    let ctx = TestContext::new();
    let height = Height::new(1);
    Driver::new(ctx, height, validator_set, my_addr, Default::default())
}

fn without_extensions(
    certificate: CommitCertificate<TestContext>,
) -> ExtendedCommitCertificate<TestContext> {
    ExtendedCommitCertificate::from_commit_certificate_and_extensions(
        certificate,
        VoteExtensions::default(),
    )
}

/// Voting power distribution used across the tests.
///
/// Total is 100; `v2` (50) + any single other non-`v1` validator (20) reaches
/// 70/100, strictly above the 2/3 threshold, so each certificate is a valid
/// polka/commit with just two signatures. Only `v2` equivocates across the two
/// certificates; the other signer in each certificate is exclusive to that
/// certificate and therefore does not equivocate.
const VOTING_POWER: [u64; 4] = [10, 50, 20, 20];

/// A polka certificate for value A and another for value B, both at the same
/// (height, round), carrying the same validator's prevote, must cause the
/// vote keeper to record equivocation evidence for that validator.
#[test]
fn polka_certificate_equivocation_is_recorded_as_evidence() {
    let [(v1, _sk1), (v2, sk2), (v3, sk3), (v4, sk4)] = make_validators(VOTING_POWER);
    let my_addr = v1.address;
    let validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone(), v4.clone()]);

    let sel = Arc::new(FixedProposer::new(my_addr));
    let height = Height::new(1);
    let round = Round::new(0);
    let value_a = Value::new(100);
    let value_b = Value::new(200);
    let proposer = sel.select_proposer(height, round, &validator_set);

    let mut driver = new_driver(my_addr, validator_set);

    driver
        .process(Input::NewRound(height, round, proposer))
        .expect("NewRound accepted");

    // cert_a: v2 and v3 prevote for A (70/100, valid polka).
    let cert_a = PolkaCertificate {
        height,
        round,
        value_id: value_a.id(),
        polka_signatures: vec![
            sign_prevote(&sk2, height, round, &value_a, v2.address),
            sign_prevote(&sk3, height, round, &value_a, v3.address),
        ],
    };
    // cert_b: v2 and v4 prevote for B (70/100, valid polka).
    // v3 is only in cert_a and v4 is only in cert_b, so only v2 equivocates.
    let cert_b = PolkaCertificate {
        height,
        round,
        value_id: value_b.id(),
        polka_signatures: vec![
            sign_prevote(&sk2, height, round, &value_b, v2.address),
            sign_prevote(&sk4, height, round, &value_b, v4.address),
        ],
    };

    driver
        .process(Input::PolkaCertificate(cert_a))
        .expect("first polka certificate accepted");
    driver
        .process(Input::PolkaCertificate(cert_b))
        .expect("second polka certificate accepted");

    let evidence = driver.votes().evidence();
    let entries = evidence
        .get(&v2.address)
        .expect("evidence recorded for equivocating validator");
    assert_eq!(entries.len(), 1, "exactly one equivocation pair");

    let (existing, conflicting) = &entries[0];
    assert_eq!(existing.validator_address(), &v2.address);
    assert_eq!(conflicting.validator_address(), &v2.address);
    assert_ne!(existing.value(), conflicting.value());

    assert!(
        evidence.get(&v3.address).is_none(),
        "no evidence for non-equivocating validator v3"
    );
    assert!(
        evidence.get(&v4.address).is_none(),
        "no evidence for non-equivocating validator v4"
    );
}

/// A commit certificate for value A and another for value B, both at the same
/// (height, round), carrying the same validator's precommit, must cause the
/// vote keeper to record equivocation evidence for that validator.
#[test]
fn commit_certificate_equivocation_is_recorded_as_evidence() {
    let [(v1, _sk1), (v2, sk2), (v3, sk3), (v4, sk4)] = make_validators(VOTING_POWER);
    let my_addr = v1.address;
    let validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone(), v4.clone()]);

    let sel = Arc::new(FixedProposer::new(my_addr));
    let height = Height::new(1);
    let round = Round::new(0);
    let value_a = Value::new(100);
    let value_b = Value::new(200);
    let proposer = sel.select_proposer(height, round, &validator_set);

    let mut driver = new_driver(my_addr, validator_set);

    driver
        .process(Input::NewRound(height, round, proposer))
        .expect("NewRound accepted");

    // cert_a: v2 and v3 precommit for A (70/100, valid commit certificate).
    let cert_a = CommitCertificate {
        height,
        round,
        value_id: value_a.id(),
        commit_signatures: vec![
            sign_precommit(&sk2, height, round, &value_a, v2.address),
            sign_precommit(&sk3, height, round, &value_a, v3.address),
        ],
    };
    // cert_b: v2 and v4 precommit for B (70/100, valid commit certificate).
    // Only v2 equivocates; v3 is exclusive to cert_a, v4 to cert_b.
    let cert_b = CommitCertificate {
        height,
        round,
        value_id: value_b.id(),
        commit_signatures: vec![
            sign_precommit(&sk2, height, round, &value_b, v2.address),
            sign_precommit(&sk4, height, round, &value_b, v4.address),
        ],
    };

    driver
        .process(Input::CommitCertificate(without_extensions(cert_a)))
        .expect("first commit certificate accepted");
    driver
        .process(Input::CommitCertificate(without_extensions(cert_b)))
        .expect("second commit certificate accepted");

    let evidence = driver.votes().evidence();
    let entries = evidence
        .get(&v2.address)
        .expect("evidence recorded for equivocating validator");
    assert_eq!(entries.len(), 1, "exactly one equivocation pair");

    let (existing, conflicting) = &entries[0];
    assert_eq!(existing.validator_address(), &v2.address);
    assert_eq!(conflicting.validator_address(), &v2.address);
    assert_ne!(existing.value(), conflicting.value());

    assert!(
        evidence.get(&v3.address).is_none(),
        "no evidence for non-equivocating validator v3"
    );
    assert!(
        evidence.get(&v4.address).is_none(),
        "no evidence for non-equivocating validator v4"
    );
}

/// A validator's precommit delivered first as a network message and later inside
/// a commit certificate for a different value must be recognized as evidence.
///
/// Additionally, a non-equivocating validator (`v3`) sends a precommit for the
/// certificate's value as a regular network vote and then appears in the
/// certificate with the same vote — this duplicate must not be recorded as
/// evidence.
#[test]
fn commit_certificate_equivocates_against_prior_network_vote() {
    let [(v1, _sk1), (v2, sk2), (v3, sk3), (v4, _sk4)] = make_validators(VOTING_POWER);
    let my_addr = v1.address;
    let validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone(), v4.clone()]);

    let sel = Arc::new(FixedProposer::new(my_addr));
    let height = Height::new(1);
    let round = Round::new(0);
    let value_a = Value::new(100);
    let value_b = Value::new(200);
    let proposer = sel.select_proposer(height, round, &validator_set);

    let mut driver = new_driver(my_addr, validator_set);
    driver
        .process(Input::NewRound(height, round, proposer))
        .expect("NewRound accepted");

    // v2 precommits for A as a network message.
    let precommit_for_a =
        Vote::new_precommit(height, round, NilOrVal::Val(value_a.id()), v2.address);
    let signed_precommit_for_a = SignedVote::new(
        precommit_for_a.clone(),
        sk2.sign(&precommit_for_a.to_sign_bytes()),
    );
    driver
        .process(Input::Vote(signed_precommit_for_a))
        .expect("network precommit for A accepted");

    // v3 precommits for B as a regular network vote. The same vote is later
    // included in cert_b below — this is a duplicate, not an equivocation.
    let precommit_for_b =
        Vote::new_precommit(height, round, NilOrVal::Val(value_b.id()), v3.address);
    let signed_precommit_for_b = SignedVote::new(
        precommit_for_b.clone(),
        sk3.sign(&precommit_for_b.to_sign_bytes()),
    );
    driver
        .process(Input::Vote(signed_precommit_for_b))
        .expect("network precommit for B accepted");

    // cert_b: v2 precommits for B (equivocates with prior network A) and
    // v3 precommits for B (duplicate of the prior network vote). 70/100 voting
    // power, valid commit certificate.
    let cert_b = CommitCertificate {
        height,
        round,
        value_id: value_b.id(),
        commit_signatures: vec![
            sign_precommit(&sk2, height, round, &value_b, v2.address),
            sign_precommit(&sk3, height, round, &value_b, v3.address),
        ],
    };
    driver
        .process(Input::CommitCertificate(without_extensions(cert_b)))
        .expect("commit certificate for B accepted");

    let evidence = driver.votes().evidence();
    let entries = evidence
        .get(&v2.address)
        .expect("evidence recorded for equivocating validator");
    assert_eq!(entries.len(), 1);
    assert!(
        evidence.get(&v3.address).is_none(),
        "no evidence for duplicate non-equivocating vote"
    );
}

/// A validator's prevote delivered first as a network message and later inside
/// a polka certificate for a different value must be recognized as evidence.
///
/// Additionally, a non-equivocating validator (`v3`) sends a prevote for the
/// certificate's value as a regular network vote and then appears in the
/// certificate with the same vote — this duplicate must not be recorded as
/// evidence.
#[test]
fn polka_certificate_equivocates_against_prior_network_vote() {
    let [(v1, _sk1), (v2, sk2), (v3, sk3), (v4, _sk4)] = make_validators(VOTING_POWER);
    let my_addr = v1.address;
    let validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone(), v4.clone()]);

    let sel = Arc::new(FixedProposer::new(my_addr));
    let height = Height::new(1);
    let round = Round::new(0);
    let value_a = Value::new(100);
    let value_b = Value::new(200);
    let proposer = sel.select_proposer(height, round, &validator_set);

    let mut driver = new_driver(my_addr, validator_set);
    driver
        .process(Input::NewRound(height, round, proposer))
        .expect("NewRound accepted");

    // v2 prevotes for A as a network message.
    let prevote_for_a = Vote::new_prevote(height, round, NilOrVal::Val(value_a.id()), v2.address);
    let signed_prevote_for_a = SignedVote::new(
        prevote_for_a.clone(),
        sk2.sign(&prevote_for_a.to_sign_bytes()),
    );
    driver
        .process(Input::Vote(signed_prevote_for_a))
        .expect("network prevote for A accepted");

    // v3 prevotes for B as a regular network vote. The same vote is later
    // included in cert_b below — this is a duplicate, not an equivocation.
    let prevote_for_b = Vote::new_prevote(height, round, NilOrVal::Val(value_b.id()), v3.address);
    let signed_prevote_for_b = SignedVote::new(
        prevote_for_b.clone(),
        sk3.sign(&prevote_for_b.to_sign_bytes()),
    );
    driver
        .process(Input::Vote(signed_prevote_for_b))
        .expect("network prevote for B accepted");

    // cert_b: v2 prevotes for B (equivocates with prior network A) and
    // v3 prevotes for B (duplicate of the prior network vote). 70/100 voting
    // power, valid polka certificate.
    let cert_b = PolkaCertificate {
        height,
        round,
        value_id: value_b.id(),
        polka_signatures: vec![
            sign_prevote(&sk2, height, round, &value_b, v2.address),
            sign_prevote(&sk3, height, round, &value_b, v3.address),
        ],
    };
    driver
        .process(Input::PolkaCertificate(cert_b))
        .expect("polka certificate for B accepted");

    let evidence = driver.votes().evidence();
    let entries = evidence
        .get(&v2.address)
        .expect("evidence recorded for equivocating validator");
    assert_eq!(entries.len(), 1);
    assert!(
        evidence.get(&v3.address).is_none(),
        "no evidence for duplicate non-equivocating vote"
    );
}
