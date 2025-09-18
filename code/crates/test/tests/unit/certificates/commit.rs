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
