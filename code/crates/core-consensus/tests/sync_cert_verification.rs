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
use malachitebft_signing::{SigningProvider, SigningProviderExt};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{
    Address, Ed25519Provider, Height, TestContext, Validator, ValidatorSet, Value,
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
    )
}

/// Build a valid commit certificate for the given height, round, and value,
/// signed by the given validators/signers.
fn build_commit_certificate(
    validators: &[Validator],
    signers: &[Ed25519Provider],
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

/// Test that the sync decision path verifies the commit certificate only once,
/// in sync.rs at the trust boundary. The certificate stored in the driver is
/// already verified and does not need re-verification in decide.rs.
#[test]
fn sync_decision_path_verifies_commit_certificate_once() {
    let entries: Vec<(Validator, _)> = make_validators([25, 25, 25, 25]).into();
    let validators: Vec<Validator> = entries.iter().map(|(v, _)| v.clone()).collect();
    let signers: Vec<Ed25519Provider> = entries
        .into_iter()
        .map(|(_, pk)| Ed25519Provider::new(pk))
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
        use Effect::*;
        Ok(match effect {
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
                verify_count.set(verify_count.get() + 1);
                let result = block_on(signers[0].verify_commit_certificate(
                    &TestContext::new(),
                    &cert,
                    &validator_set,
                    tp,
                ));
                r.resume_with(result)
            }
            _ => Resume::Continue,
        })
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
