use async_trait::async_trait;
use malachitebft_core_types::PolkaCertificate;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct Polka;

#[async_trait]
impl CertificateBuilder for Polka {
    type Certificate = PolkaCertificate<TestContext>;

    async fn build_certificate(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        let value_id = value_id.expect("value_id must be Some(_) in polka certificate");
        PolkaCertificate::new(height, round, value_id, votes)
    }

    async fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        signer
            .verify_polka_certificate(ctx, certificate, validator_set, threshold_params)
            .await
    }
}

/// Tests the verification of a valid PolkaCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[tokio::test]
async fn valid_polka_certificate_with_sufficient_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[tokio::test]
async fn valid_polka_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([21, 22, 24, 30])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([21, 22, 24, 0])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
#[tokio::test]
async fn invalid_polka_certificate_insufficient_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        })
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 30, 50])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        })
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 30, 50])
        .with_nil_votes(0..4, VoteType::Prevote)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        })
        .await;
}

/// Tests the verification of a certificate containing multiple votes from the same validator.
#[tokio::test]
async fn invalid_polka_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[2].address
    };

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr))
        .await;
}

/// Tests the verification of a certificate containing a vote from a validator not in the validator set.
#[tokio::test]
async fn invalid_polka_certificate_unknown_validator() {
    // Define the seed for generating the other validator twice
    let seed = 0xcafecafe;

    let external_validator_addr = {
        let ([validator], _) = make_validators([0], seed);
        validator.address
    };

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .with_non_validator_vote(seed, VoteType::Prevote)
        .await
        .expect_error(CertificateError::UnknownValidator(external_validator_addr))
        .await;
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[tokio::test]
async fn invalid_polka_certificate_invalid_signature() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_signature_vote(2, VoteType::Prevote) // Validator 2 has invalid signature
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        })
        .await;
}

/// Tests the verification of a certificate containing a vote with invalid height or round.
#[tokio::test]
async fn invalid_polka_certificate_wrong_vote_height_round() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_height_vote(2, VoteType::Prevote) // Validator 2 has invalid vote height
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        })
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_round_vote(2, VoteType::Prevote) // Validator 2 has invalid vote round
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        })
        .await;
}

/// Tests the verification of a certificate with no votes.
#[tokio::test]
async fn empty_polka_certificate() {
    CertificateTest::<Polka>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Prevote) // No signatures
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 3,
        })
        .await;
}

/// Tests the verification of a certificate containing both valid and invalid votes.
#[tokio::test]
async fn polka_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Prevote)
        .await
        .with_invalid_signature_vote(0, VoteType::Prevote)
        .await // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Prevote)
        .await // Invalid signature for validator 1
        .expect_valid()
        .await;

    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_signature_vote(2, VoteType::Prevote)
        .await // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Prevote) // Invalid signature for validator 3
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        })
        .await;
}
