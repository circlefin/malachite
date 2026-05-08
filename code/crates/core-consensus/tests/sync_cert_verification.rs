/// Verify that the sync decision path emits `VerifyCommitCertificate` exactly once.
///
/// The certificate is verified in `sync.rs::process_commit_certificate` at the
/// trust boundary. When `decide.rs` retrieves the same certificate from the driver,
/// it skips re-verification since it was already verified before being stored.
use std::cell::Cell;

use arc_malachitebft_core_consensus::{
    process, Effect, Error, Input, Params, ProposedValue, Resumable, Resume, State,
};
use malachitebft_core_types::{
    CommitCertificate, CommitSignature, Context, NilOrVal, Round, Validity, ValueOrigin,
    ValuePayload, ValueResponse,
};
use malachitebft_metrics::Metrics;
use malachitebft_peer::PeerId;
use malachitebft_signing::{Signer, VerifierExt};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Ed25519Signer, Height, TestContext, Validator, ValidatorSet, Value,
};

use bytes::Bytes;
use futures::executor::block_on;

fn run(r: Result<(), Error<TestContext>>) {
    drop(r);
}

fn make_state(validators: &[Validator], my_addr: Address) -> State<TestContext> {
    let vs = ValidatorSet::new(validators.to_vec());
    State::new(
        TestContext::new(),
        Height::new(1),
        vs.clone(),
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

/// Build a valid commit certificate for the given height, round, and value,
/// signed by the given validators/signers.
fn build_commit_certificate(
    validators: &[Validator],
    signers: &[Ed25519Signer],
    height: Height,
    round: Round,
    value: &Value,
) -> CommitCertificate<TestContext> {
    let ctx = TestContext::new();
    let value_id = value.id();

    let commit_signatures: Vec<_> = validators
        .iter()
        .zip(signers.iter())
        .map(|(v, signer)| {
            let vote = ctx.new_precommit(height, round, NilOrVal::Val(value_id), v.address);
            let signed = block_on(signer.sign_vote(vote)).unwrap();
            CommitSignature::new(v.address, signed.signature)
        })
        .collect();

    CommitCertificate {
        height,
        round,
        value_id,
        commit_signatures,
    }
}

/// Handle the effects emitted by the consensus core during these tests:
/// signature verification, signing, and commit-certificate verification.
///
/// All signing/verification is performed using `signers[0]`. If `verify_cert_count` is
/// `Some`, it is incremented every time a `VerifyCommitCertificate` effect is observed.
///
/// Effects not handled here are forwarded to `fallback` so the caller can apply
/// test-specific logic.
fn handle_effect_with_default<F>(
    signers: &[Ed25519Signer],
    verify_cert_count: Option<&Cell<u32>>,
    effect: Effect<TestContext>,
    fallback: F,
) -> Resume<TestContext>
where
    F: FnOnce(Effect<TestContext>) -> Resume<TestContext>,
{
    use Effect::*;
    match effect {
        VerifySignature(_, _, r) => r.resume_with(true),
        SignVote(vote, r) => {
            let signed = block_on(signers[0].sign_vote(vote)).unwrap();
            r.resume_with(signed)
        }
        SignProposal(proposal, r) => {
            let signed = block_on(signers[0].sign_proposal(proposal)).unwrap();
            r.resume_with(signed)
        }
        VerifyCommitCertificate(cert, validator_set, tp, r) => {
            if let Some(counter) = verify_cert_count {
                counter.set(counter.get() + 1);
            }
            let result = block_on(signers[0].verify_commit_certificate(
                &TestContext::new(),
                &cert,
                &validator_set,
                tp,
            ));
            r.resume_with(result)
        }
        other => fallback(other),
    }
}

/// Test that the sync decision path verifies the commit certificate only once,
/// in sync.rs at the trust boundary. The certificate stored in the driver is
/// already verified and does not need re-verification in decide.rs.
#[test]
fn sync_decision_path_verifies_commit_certificate_once() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();
    let signers: Vec<Ed25519Signer> = entries
        .into_iter()
        .map(|(_, pk)| Ed25519Signer::new(pk))
        .collect();

    // We are validator 0 (also the proposer for height 1, round 0)
    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value = Value::new(42);

    // Counter for VerifyCommitCertificate effects
    let verify_count = Cell::new(0u32);

    let handle_effect = |effect: Effect<TestContext>| -> Result<Resume<TestContext>, ()> {
        Ok(handle_effect_with_default(
            &signers,
            Some(&verify_count),
            effect,
            |_| Resume::Continue,
        ))
    };

    // Step 1: Start height
    run(process!(
        input: Input::StartHeight(height, vs, false, None),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    assert_eq!(verify_count.get(), 0, "No verification before sync");

    // Step 2: Build and send a valid commit certificate via sync
    let certificate = build_commit_certificate(&validators, &signers, height, round, &value);
    let value_response =
        ValueResponse::new(PeerId::random(), Bytes::from("value-bytes"), certificate);

    run(process!(
        input: Input::SyncValueResponse(value_response),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    assert_eq!(
        verify_count.get(),
        1,
        "Certificate verified once in sync.rs::process_commit_certificate"
    );

    // Step 3: Provide the proposed value (from sync origin) to trigger maybe_sync_decision
    run(process!(
        input: Input::ProposedValue(
            ProposedValue {
                height,
                round,
                valid_round: Round::Nil,
                proposer: my_addr,
                value: value.clone(),
                validity: Validity::Valid,
            },
            ValueOrigin::Sync,
        ),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    // Certificate is verified only once on the sync path: sync.rs verifies at the
    // trust boundary, decide.rs reuses the already-verified certificate from the driver.
    assert_eq!(
        verify_count.get(),
        1,
        "Certificate should be verified only once on sync decision path"
    );
}

/// When a sync `ValueResponse` is processed and the matching `ProposedValue` is already
/// known locally (e.g., from WAL replay or via the consensus race), `process_commit_certificate`
/// drives the state machine to a decision. In that case there is no point in forwarding the
/// raw value bytes to the host via `Effect::ValidSyncValue` as the host has already accepted
/// the value through the local proposed-value path. This test asserts that `ValidSyncValue`
/// is not emitted in that case.
#[test]
fn sync_value_response_skips_valid_sync_value_when_already_decided() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();
    let signers: Vec<Ed25519Signer> = entries
        .into_iter()
        .map(|(_, pk)| Ed25519Signer::new(pk))
        .collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value = Value::new(42);

    let valid_sync_value_count = Cell::new(0u32);
    let invalid_sync_value_count = Cell::new(0u32);

    let handle_effect = |effect: Effect<TestContext>| -> Result<Resume<TestContext>, ()> {
        Ok(handle_effect_with_default(
            &signers,
            None,
            effect,
            |effect| {
                use Effect::*;
                match effect {
                    ValidSyncValue(_, _, r) => {
                        valid_sync_value_count.set(valid_sync_value_count.get() + 1);
                        r.resume_with(())
                    }
                    InvalidSyncValue(_, _, _, r) => {
                        invalid_sync_value_count.set(invalid_sync_value_count.get() + 1);
                        r.resume_with(())
                    }
                    _ => Resume::Continue,
                }
            },
        ))
    };

    run(process!(
        input: Input::StartHeight(height, vs, false, None),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    // Step 1: Provide the proposed value first (simulates WAL-replayed/locally-stored value)
    run(process!(
        input: Input::ProposedValue(
            ProposedValue {
                height,
                round,
                valid_round: Round::Nil,
                proposer: my_addr,
                value: value.clone(),
                validity: Validity::Valid,
            },
            ValueOrigin::Consensus,
        ),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    // Step 2: The sync value response arrives with the matching certificate.
    // Since the value is already stored, `process_commit_certificate` should drive
    // the state machine straight to a decision, and `ValidSyncValue` must be skipped.
    let certificate = build_commit_certificate(&validators, &signers, height, round, &value);
    let value_response =
        ValueResponse::new(PeerId::random(), Bytes::from("value-bytes"), certificate);

    run(process!(
        input: Input::SyncValueResponse(value_response),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    assert_eq!(
        valid_sync_value_count.get(),
        0,
        "ValidSyncValue must not be emitted when the certificate processing already decided"
    );
    assert_eq!(
        invalid_sync_value_count.get(),
        0,
        "InvalidSyncValue must not be emitted on a successful sync decision"
    );
}

/// Sanity-check the inverse: when no matching `ProposedValue` is stored locally,
/// `process_commit_certificate` cannot reach a decision on its own and `ValidSyncValue`
/// must still be emitted so the host can decode the value bytes.
#[test]
fn sync_value_response_emits_valid_sync_value_when_not_decided() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();
    let signers: Vec<Ed25519Signer> = entries
        .into_iter()
        .map(|(_, pk)| Ed25519Signer::new(pk))
        .collect();

    let my_addr = validators[0].address;
    let mut state = make_state(&validators, my_addr);
    let metrics = Metrics::new();
    let vs = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value = Value::new(42);

    let valid_sync_value_count = Cell::new(0u32);

    let handle_effect = |effect: Effect<TestContext>| -> Result<Resume<TestContext>, ()> {
        Ok(handle_effect_with_default(
            &signers,
            None,
            effect,
            |effect| {
                use Effect::*;
                match effect {
                    ValidSyncValue(_, _, r) => {
                        valid_sync_value_count.set(valid_sync_value_count.get() + 1);
                        r.resume_with(())
                    }
                    _ => Resume::Continue,
                }
            },
        ))
    };

    run(process!(
        input: Input::StartHeight(height, vs, false, None),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    let certificate = build_commit_certificate(&validators, &signers, height, round, &value);
    let value_response =
        ValueResponse::new(PeerId::random(), Bytes::from("value-bytes"), certificate);

    run(process!(
        input: Input::SyncValueResponse(value_response),
        state: &mut state,
        metrics: &metrics,
        with: effect => handle_effect(effect)
    ));

    assert_eq!(
        valid_sync_value_count.get(),
        1,
        "ValidSyncValue must be emitted when no decision was reached during certificate processing"
    );
}
