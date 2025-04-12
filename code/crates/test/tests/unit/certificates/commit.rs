use malachitebft_core_types::CommitCertificate;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct Commit;

impl CertificateBuilder for Commit {
    type Certificate = CommitCertificate<TestContext>;

    fn build_certificate(
        height: Height,
        round: Round,
        value_id: ValueId,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        CommitCertificate::new(height, round, value_id, votes)
    }

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        signer.verify_commit_certificate(ctx, certificate, validator_set, threshold_params)
    }

    fn make_vote(
        ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        validator_address: Address,
    ) -> Vote {
        ctx.new_precommit(height, round, value_id, validator_address)
    }
}

/// Tests the verification of a valid CommitCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_commit_certificate_with_sufficient_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 30, 30])
        .with_signatures(0..4)
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([20, 20, 30, 30])
        .with_signatures(0..3)
        .expect_valid();
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_commit_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([21, 22, 24, 30])
        .with_signatures(0..3)
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([21, 22, 24, 0])
        .with_signatures(0..3)
        .expect_valid();
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_commit_certificate_insufficient_voting_power() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_signatures(0..3)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 30, 50])
        .with_signatures(0..2)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 30, 50])
        .with_signatures(0..4)
        .all_vote_nil()
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
        validators[0].address
    };

    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10, 10])
        .with_signatures(0..4)
        .with_duplicate_vote(0) // Add duplicate vote from validator 0
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
        .with_signatures(0..4)
        .with_external_vote(seed)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[test]
fn invalid_commit_certificate_invalid_signature_1() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10])
        .with_signatures(0..3)
        .with_invalid_signature(0) // Validator 0 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[test]
fn invalid_commit_certificate_invalid_signature_2() {
    CertificateTest::<Commit>::new()
        .with_validators([10, 10, 10])
        .with_signatures(0..3)
        .with_invalid_signature(0) // Replace signature for validator 0
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
        .with_signatures([]) // No signatures
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
        .with_signatures(0..4)
        .with_invalid_signature(0) // Invalid signature for validator 0
        .with_invalid_signature(1) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::<Commit>::new()
        .with_validators([10, 20, 30, 40])
        .with_signatures(0..4)
        .with_invalid_signature(2) // Invalid signature for validator 2
        .with_invalid_signature(3) // Invalid signature for validator 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        });
}
