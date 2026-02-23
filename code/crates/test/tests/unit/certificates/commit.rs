use futures::executor::block_on;
use malachitebft_core_types::CommitCertificate;
use malachitebft_signing::SigningProviderExt;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct Commit;

impl CertificateBuilder for Commit {
    type Certificate = CommitCertificate<TestContext>;

    fn build_certificate(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        let value_id = value_id.expect("value_id must be Some(_) in commit certificate");
        CommitCertificate::new(height, round, value_id, votes)
    }

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        block_on(signer.verify_commit_certificate(
            ctx,
            certificate,
            validator_set,
            threshold_params,
        ))
    }
}

/// Tests the verification of a valid CommitCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_commit_certificate_with_sufficient_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_commit_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([21, 22, 24, 30])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([21, 22, 24, 0])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_commit_certificate_insufficient_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..3, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 30, 50])
        .with_votes(0..2, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 30, 50])
        .with_nil_votes(0..4, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        });
}

/// Tests the verification of a certificate containing multiple votes from the same validator.
#[test]
fn invalid_commit_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[3].address
    };

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..4, VoteType::Precommit)
        .with_duplicate_last_vote() // Add duplicate last vote
        .expect_error(CertificateError::DuplicateVote(validator_addr));
}

/// Tests the verification of a certificate containing a vote from a validator not in the validator set.
#[test]
fn invalid_commit_certificate_unknown_validator() {
    // Define the seed for generating the other validator twice
    let seed = 0xcafecafe;

    let external_validator_addr = {
        let ([validator], _) = make_validators([0], seed);
        validator.address
    };

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Precommit)
        .with_non_validator_vote(seed, VoteType::Precommit)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[test]
fn invalid_commit_certificate_invalid_signature_1() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_signature_vote(2, VoteType::Precommit) // Validator 0 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate with no votes.
#[test]
fn empty_commit_certificate() {
    CertificateTest::<Commit>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Precommit) // No signatures
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 3,
        });
}

/// Tests the verification of a certificate containing both valid and invalid votes.
#[test]
fn commit_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Precommit)
        .with_invalid_signature_vote(0, VoteType::Precommit) // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Precommit) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_signature_vote(2, VoteType::Precommit) // Invalid signature for validator 0
        .with_invalid_signature_vote(3, VoteType::Precommit) // Invalid signature for validator 1
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        });
}

/// Tests extended certificate.
#[test]
fn valid_extended_commit_certificate() {
    // Minimal certificate
    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 20, 20, 20, 20, 20])
        .with_votes(0..5, VoteType::Precommit)
        .expect_valid();

    // Extended certificate
    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 20, 20, 20, 20, 20])
        .with_votes(1..7, VoteType::Precommit)
        .expect_valid();

    // Full certificate
    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 20, 20, 20, 20, 20])
        .with_votes(0..7, VoteType::Precommit)
        .expect_valid();

    // Extended certificate with varied weights; total VP: 100
    CertificateTest::<Commit>::new()
        .with_validators([10, 15, 20, 25, 30])
        .with_votes(1..5, VoteType::Precommit) // validator 1 not needed
        .expect_valid();
}

// ============================================================================
// Security-focused tests: address spoofing, signature replay, validator set
// mismatch, and quorum boundary conditions.
// ============================================================================

/// Address spoofing attack: a spoofed signature claims to be from a high-VP validator
/// but was actually signed by a different validator's key. Malachite looks up validators
/// by address and verifies against the looked-up validator's public key, so the spoofed
/// signature fails verification and contributes no voting power.
#[test]
fn commit_certificate_address_spoofing_attack() {
    // Validators: [10, 90]. Spoofed sig claims to be validator 1 (VP=90)
    // but is signed by validator 0's key. Signature fails → VP counted = 0.
    CertificateTest::<Commit>::new()
        .with_validators([10, 90])
        .with_spoofed_address_vote(1, 0, VoteType::Precommit) // claims idx 1, signed by idx 0
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        });
}

/// Address spoofing mixed with valid votes: legitimate votes contribute their VP,
/// but the spoofed vote contributes nothing.
#[test]
fn commit_certificate_address_spoofing_with_valid_votes() {
    // Validators: [10, 20, 30, 40]. Validators 0-1 sign legitimately (VP=30).
    // Spoofed sig claims validator 3 (VP=40) but signed by validator 2's key.
    // Only VP=30 counted, which is insufficient.
    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .with_spoofed_address_vote(3, 2, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        });
}

/// Signature replay across heights: valid precommit signatures from height 1
/// are injected into a certificate at height 2. The verifier reconstructs votes
/// at height 2, which doesn't match what was signed at height 1.
#[test]
fn commit_certificate_signature_replay_across_heights() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height_1 = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign valid precommits at height 1
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_precommit(
                height_1,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Extract signatures, inject into certificate at height 2
    let certificate = CommitCertificate {
        height: Height::new(2),
        round,
        value_id,
        commit_signatures: votes
            .iter()
            .map(|v| CommitSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_commit_certificate(
        &ctx,
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    ));
    assert_eq!(
        result,
        Err(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        })
    );
}

/// Signature replay across rounds: valid precommit signatures from round 0
/// are injected into a certificate at round 1.
#[test]
fn commit_certificate_signature_replay_across_rounds() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round_0 = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign valid precommits at round 0
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_precommit(
                height,
                round_0,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject into certificate at round 1
    let certificate = CommitCertificate {
        height,
        round: Round::new(1),
        value_id,
        commit_signatures: votes
            .iter()
            .map(|v| CommitSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_commit_certificate(
        &ctx,
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    ));
    assert_eq!(
        result,
        Err(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        })
    );
}

/// Signature replay across values: valid precommit signatures for value 42
/// are injected into a certificate for value 99.
#[test]
fn commit_certificate_signature_replay_across_values() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);

    // Sign valid precommits for value 42
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_precommit(
                height,
                round,
                NilOrVal::Val(ValueId::new(42)),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject into certificate for value 99
    let certificate = CommitCertificate {
        height,
        round,
        value_id: ValueId::new(99),
        commit_signatures: votes
            .iter()
            .map(|v| CommitSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_commit_certificate(
        &ctx,
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    ));
    assert_eq!(
        result,
        Err(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        })
    );
}

/// Validator set mismatch: signatures from validator set A are verified
/// against validator set B. All addresses are unknown.
#[test]
fn commit_certificate_validator_set_mismatch() {
    let (validators_a, signers_a) = make_validators([25, 25, 25, 25], 0xAAAA);
    let (validators_b, _) = make_validators([25, 25, 25, 25], 0xBBBB);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign with set A's keys
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers_a[i].sign_vote(ctx.new_precommit(
                height,
                round,
                NilOrVal::Val(value_id),
                validators_a[i].address,
            )))
            .unwrap()
        })
        .collect();

    let certificate = CommitCertificate::new(height, round, value_id, votes);

    // Verify against set B
    let validator_set_b = ValidatorSet::new(validators_b.to_vec());
    let result = block_on(signers_a[0].verify_commit_certificate(
        &ctx,
        &certificate,
        &validator_set_b,
        ThresholdParams::default(),
    ));
    assert!(matches!(result, Err(CertificateError::UnknownValidator(_))));
}

/// Quorum boundary: exactly 2/3 is NOT sufficient because the check is strict >
/// (signed * 3 > total * 2). With validators [1, 1, 1] and 2 of 3 signing:
/// 2*3=6 > 3*2=6 is false, so the quorum is not met.
#[test]
fn commit_certificate_quorum_boundary_exact_two_thirds_insufficient() {
    CertificateTest::<Commit>::new()
        .with_validators([1, 1, 1])
        .with_votes(0..2, VoteType::Precommit) // VP=2 out of 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 2,
            total: 3,
            expected: 3,
        });
}

/// Quorum boundary: just above 2/3. With validators [34, 33, 33] and all signing:
/// 100*3=300 > 100*2=200, yes → valid. Also signing just [0, 1] (VP=67):
/// 67*3=201 > 100*2=200, yes → valid.
#[test]
fn commit_certificate_quorum_boundary_just_above_two_thirds_sufficient() {
    // All three sign (VP=100)
    CertificateTest::<Commit>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    // Only validators 0 and 1 sign (VP=67)
    CertificateTest::<Commit>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..2, VoteType::Precommit)
        .expect_valid();
}
