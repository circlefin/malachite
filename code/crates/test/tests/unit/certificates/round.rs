use async_trait::async_trait;
use malachitebft_core_types::RoundCertificate;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct RoundSkip;
pub struct RoundPrecommit;

#[async_trait]
impl CertificateBuilder for RoundSkip {
    type Certificate = RoundCertificate<TestContext>;

    async fn build_certificate(
        height: Height,
        round: Round,
        _value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Skip, votes)
    }

    async fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        signer
            .verify_round_certificate(ctx, certificate, validator_set, threshold_params)
            .await
    }
}

#[async_trait]
impl CertificateBuilder for RoundPrecommit {
    type Certificate = RoundCertificate<TestContext>;

    async fn build_certificate(
        height: Height,
        round: Round,
        _value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Precommit, votes)
    }

    async fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        signer
            .verify_round_certificate(ctx, certificate, validator_set, threshold_params)
            .await
    }
}

/// Tests the verification of a valid SkipRoundCertificate with signatures from validators
/// representing more than 1/3 of the total voting power.
#[tokio::test]
async fn valid_round_skip_certificate_with_sufficient_voting_power() {
    // SkipRoundCertificate from prevotes
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    // SkipRoundCertificate from precommits
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    // SkipRoundCertificate from prevotes nil
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..2, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    // SkipRoundCertificate from precommits nil
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..2, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    // SkipRoundCertificate from mixed votes
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .await
        .with_nil_votes(1..3, VoteType::Prevote)
        .await
        .with_different_value_vote(3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..1, VoteType::Precommit)
        .await
        .with_votes(1..2, VoteType::Prevote)
        .await
        .with_different_value_vote(2, VoteType::Prevote)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .await
        .with_different_value_vote(1, VoteType::Prevote)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a valid SkipRoundCertificate with signatures from validators
/// representing more than 1/3 of the total voting power with random mixed votes.
#[tokio::test]
async fn valid_round_skip_certificate_with_mixed_votes_with_sufficient_voting_power() {
    for _ in 0..1000 {
        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..4, None)
            .await
            .expect_valid()
            .await;

        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..3, None)
            .await
            .expect_valid()
            .await;

        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..2, None)
            .await
            .expect_valid()
            .await;
    }
}

/// Tests the verification of a valid PrecommitRoundCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[tokio::test]
async fn valid_round_precommit_certificate_with_sufficient_voting_power() {
    // PrecommitRoundCertificate from precommits
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    // PrecommitRoundCertificate from precommits nil
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    // PrecommitRoundCertificate from mixed precommits
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .with_nil_votes(2..3, VoteType::Precommit)
        .await
        .with_different_value_vote(3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .await
        .with_nil_votes(1..2, VoteType::Precommit)
        .await
        .with_different_value_vote(2, VoteType::Precommit)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a valid PrecommitRoundCertificate with signatures from validators
/// representing more than 2/3 of the total voting power with random mixed votes.
#[tokio::test]
async fn valid_round_precommit_certificate_with_mixed_votes_with_sufficient_voting_power() {
    for _ in 0..1000 {
        CertificateTest::<RoundPrecommit>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..4, Some(VoteType::Precommit))
            .await
            .expect_valid()
            .await;

        CertificateTest::<RoundPrecommit>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..3, Some(VoteType::Precommit))
            .await
            .expect_valid()
            .await;
    }
}

/// Tests the verification of a skip round certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[tokio::test]
async fn valid_round_skip_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([12, 21, 29, 35])
        .with_votes(0..1, VoteType::Prevote)
        .await
        .with_nil_votes(1..2, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([23, 19, 25, 0])
        .with_votes(0..1, VoteType::Prevote)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a precommit round certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[tokio::test]
async fn valid_round_precommit_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([15, 19, 31, 32])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .expect_valid()
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([30, 36, 16, 15])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .expect_valid()
        .await;
}

/// Tests the verification of a skip round certificate with valid signatures but insufficient voting power.
#[tokio::test]
async fn invalid_round_skip_certificate_insufficient_voting_power() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 5, 10, 75])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 25,
            total: 100,
            expected: 34,
        })
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 30, 50])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 34,
        })
        .await;
}

/// Tests the verification of a precommit round certificate with valid signatures but insufficient voting power.
#[tokio::test]
async fn invalid_round_precommit_certificate_insufficient_voting_power() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 30, 10, 50])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 50,
            total: 100,
            expected: 67,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([30, 36, 0, 34])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 66,
            total: 100,
            expected: 67,
        })
        .await;
}

/// Tests the verification of a round certificate containing multiple votes from the same validator.
#[tokio::test]
async fn invalid_round_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[2].address
    };

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr))
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr))
        .await;
}

/// Tests the verification of a round certificate containing a vote from a validator not in the validator set.
#[tokio::test]
async fn invalid_round_certificate_unknown_validator() {
    let seed = 0xcafecafe;

    let external_validator_addr = {
        let ([validator], _) = make_validators([0], seed);
        validator.address
    };

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .await
        .with_non_validator_vote(seed, VoteType::Prevote)
        .await
        .expect_error(CertificateError::UnknownValidator(external_validator_addr))
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Precommit)
        .await
        .with_non_validator_vote(seed, VoteType::Prevote)
        .await
        .expect_error(CertificateError::UnknownValidator(external_validator_addr))
        .await;
}

/// Tests the verification of a round certificate containing a vote with an invalid signature.
#[tokio::test]
async fn invalid_round_certificate_invalid_signature() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 5, 5])
        .with_votes(1..3, VoteType::Precommit)
        .await
        .with_invalid_signature_vote(0, VoteType::Precommit)
        .await // Validator 0 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(1..3, VoteType::Precommit)
        .await
        .with_invalid_signature_vote(0, VoteType::Precommit)
        .await // Validator 0 has invalid signature
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
    CertificateTest::<RoundSkip>::new()
        .with_validators([5, 5, 20])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_height_vote(2, VoteType::Prevote)
        .await // Validator 2 has invalid vote height
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        })
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([5, 5, 20])
        .with_votes(0..2, VoteType::Prevote)
        .await
        .with_invalid_round_vote(2, VoteType::Prevote)
        .await // Validator 2 has invalid vote round
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .with_invalid_height_vote(2, VoteType::Precommit)
        .await // Validator 2 has invalid vote height
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .with_invalid_round_vote(2, VoteType::Precommit)
        .await // Validator 2 has invalid vote round
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        })
        .await;
}

/// Tests the verification of a certificate with no votes.
#[tokio::test]
async fn empty_round_certificate() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Prevote)
        .await // No signatures
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 2,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Precommit)
        .await // No signatures
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 3,
        })
        .await;
}

/// Tests the verification of a certificate containing both valid and invalid votes.
#[tokio::test]
async fn round_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Prevote)
        .await
        .with_invalid_signature_vote(0, VoteType::Prevote)
        .await // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Prevote)
        .await // Invalid signature for validator 1
        .expect_valid()
        .await;

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .with_invalid_signature_vote(2, VoteType::Precommit)
        .await // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Precommit)
        .await // Invalid signature for validator 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 34,
        })
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Precommit)
        .await
        .with_invalid_signature_vote(0, VoteType::Precommit)
        .await // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Precommit)
        .await // Invalid signature for validator 1
        .expect_valid()
        .await;

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .await
        .with_invalid_signature_vote(2, VoteType::Precommit)
        .await // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Precommit)
        .await // Invalid signature for validator 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        })
        .await;
}
