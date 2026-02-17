use futures::executor::block_on;
use malachitebft_core_types::RoundCertificate;
use malachitebft_signing::SigningProviderExt;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct RoundSkip;
pub struct RoundPrecommit;

impl CertificateBuilder for RoundSkip {
    type Certificate = RoundCertificate<TestContext>;

    fn build_certificate(
        height: Height,
        round: Round,
        _value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Skip, votes)
    }

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        block_on(signer.verify_round_certificate(ctx, certificate, validator_set, threshold_params))
    }
}

impl CertificateBuilder for RoundPrecommit {
    type Certificate = RoundCertificate<TestContext>;

    fn build_certificate(
        height: Height,
        round: Round,
        _value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Precommit, votes)
    }

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        block_on(signer.verify_round_certificate(ctx, certificate, validator_set, threshold_params))
    }
}

/// Tests the verification of a valid SkipRoundCertificate with signatures from validators
/// representing more than 1/3 of the total voting power.
#[test]
fn valid_round_skip_certificate_with_sufficient_voting_power() {
    // SkipRoundCertificate from prevotes
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Prevote)
        .expect_valid();

    // SkipRoundCertificate from precommits
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Precommit)
        .expect_valid();

    // SkipRoundCertificate from prevotes nil
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..2, VoteType::Prevote)
        .expect_valid();

    // SkipRoundCertificate from precommits nil
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..2, VoteType::Precommit)
        .expect_valid();

    // SkipRoundCertificate from mixed votes
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .with_nil_votes(1..3, VoteType::Prevote)
        .with_different_value_vote(3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..1, VoteType::Precommit)
        .with_votes(1..2, VoteType::Prevote)
        .with_different_value_vote(2, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .with_different_value_vote(1, VoteType::Prevote)
        .expect_valid();
}

/// Tests the verification of a valid SkipRoundCertificate with signatures from validators
/// representing more than 1/3 of the total voting power with random mixed votes.
#[test]
fn valid_round_skip_certificate_with_mixed_votes_with_sufficient_voting_power() {
    for _ in 0..1000 {
        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..4, None)
            .expect_valid();

        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..3, None)
            .expect_valid();

        CertificateTest::<RoundSkip>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..2, None)
            .expect_valid();
    }
}

/// Tests the verification of a valid PrecommitRoundCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_round_precommit_certificate_with_sufficient_voting_power() {
    // PrecommitRoundCertificate from precommits
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    // PrecommitRoundCertificate from precommits nil
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..4, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_nil_votes(0..3, VoteType::Precommit)
        .expect_valid();

    // PrecommitRoundCertificate from mixed precommits
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..2, VoteType::Precommit)
        .with_nil_votes(2..3, VoteType::Precommit)
        .with_different_value_vote(3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..1, VoteType::Precommit)
        .with_nil_votes(1..2, VoteType::Precommit)
        .with_different_value_vote(2, VoteType::Precommit)
        .expect_valid();
}

/// Tests the verification of a valid PrecommitRoundCertificate with signatures from validators
/// representing more than 2/3 of the total voting power with random mixed votes.
#[test]
fn valid_round_precommit_certificate_with_mixed_votes_with_sufficient_voting_power() {
    for _ in 0..1000 {
        CertificateTest::<RoundPrecommit>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..4, Some(VoteType::Precommit))
            .expect_valid();

        CertificateTest::<RoundPrecommit>::new()
            .with_validators([20, 20, 30, 30])
            .with_random_votes(0..3, Some(VoteType::Precommit))
            .expect_valid();
    }
}

/// Tests the verification of a skip round certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_round_skip_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([12, 21, 29, 35])
        .with_votes(0..1, VoteType::Prevote)
        .with_nil_votes(1..2, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([23, 19, 25, 0])
        .with_votes(0..1, VoteType::Prevote)
        .expect_valid();
}

/// Tests the verification of a precommit round certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_round_precommit_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([15, 19, 31, 32])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([30, 36, 16, 15])
        .with_votes(0..2, VoteType::Precommit)
        .expect_valid();
}

/// Tests the verification of a skip round certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_round_skip_certificate_insufficient_voting_power() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 5, 10, 75])
        .with_votes(0..3, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 25,
            total: 100,
            expected: 34,
        });

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 30, 50])
        .with_votes(0..2, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 34,
        });
}

/// Tests the verification of a precommit round certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_round_precommit_certificate_insufficient_voting_power() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 30, 10, 50])
        .with_votes(0..3, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 50,
            total: 100,
            expected: 67,
        });

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([30, 36, 0, 34])
        .with_votes(0..2, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 66,
            total: 100,
            expected: 67,
        });
}

/// Tests the verification of a round certificate containing multiple votes from the same validator.
#[test]
fn invalid_round_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[2].address
    };

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr));

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Precommit)
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr));
}

/// Tests the verification of a round certificate containing a vote from a validator not in the validator set.
#[test]
fn invalid_round_certificate_unknown_validator() {
    let seed = 0xcafecafe;

    let external_validator_addr = {
        let ([validator], _) = make_validators([0], seed);
        validator.address
    };

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .with_non_validator_vote(seed, VoteType::Prevote)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Precommit)
        .with_non_validator_vote(seed, VoteType::Prevote)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));
}

/// Tests the verification of a round certificate containing a vote with an invalid signature.
#[test]
fn invalid_round_certificate_invalid_signature() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([20, 5, 5])
        .with_votes(1..3, VoteType::Precommit)
        .with_invalid_signature_vote(0, VoteType::Precommit) // Validator 0 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        });

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(1..3, VoteType::Precommit)
        .with_invalid_signature_vote(0, VoteType::Precommit) // Validator 0 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate containing a vote with invalid height or round.
#[test]
fn invalid_polka_certificate_wrong_vote_height_round() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([5, 5, 20])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_height_vote(2, VoteType::Prevote) // Validator 2 has invalid vote height
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        });

    CertificateTest::<RoundSkip>::new()
        .with_validators([5, 5, 20])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_round_vote(2, VoteType::Prevote) // Validator 2 has invalid vote round
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 10,
            total: 30,
            expected: 11,
        });

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_height_vote(2, VoteType::Precommit) // Validator 2 has invalid vote height
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_round_vote(2, VoteType::Precommit) // Validator 2 has invalid vote round
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate with no votes.
#[test]
fn empty_round_certificate() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Prevote) // No signatures
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 2,
        });

    CertificateTest::<RoundPrecommit>::new()
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
fn round_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Prevote)
        .with_invalid_signature_vote(0, VoteType::Prevote) // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Prevote) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_signature_vote(2, VoteType::Precommit) // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Precommit) // Invalid signature for validator 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 34,
        });

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Precommit)
        .with_invalid_signature_vote(0, VoteType::Precommit) // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Precommit) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Precommit)
        .with_invalid_signature_vote(2, VoteType::Precommit) // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Precommit) // Invalid signature for validator 3
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 30,
            total: 100,
            expected: 67,
        });
}

// ============================================================================
// Security-focused tests: address spoofing, signature replay, cross-type replay,
// validator set mismatch, and quorum boundary conditions.
// ============================================================================

/// Address spoofing in a SkipRound certificate (1/3+ threshold).
#[test]
fn round_skip_certificate_address_spoofing_attack() {
    // Validators: [10, 90]. Spoofed sig claims validator 1 (VP=90)
    // but is signed by validator 0's key. Sig fails → VP=0.
    CertificateTest::<RoundSkip>::new()
        .with_validators([10, 90])
        .with_spoofed_address_vote(1, 0, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 34,
        });
}

/// Address spoofing in a PrecommitRound certificate (2/3+ threshold).
#[test]
fn round_precommit_certificate_address_spoofing_attack() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([10, 90])
        .with_spoofed_address_vote(1, 0, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        });
}

/// Signature replay across heights in a SkipRound certificate.
#[test]
fn round_skip_certificate_signature_replay_across_heights() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height_1 = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign valid prevotes at height 1
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_prevote(
                height_1,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject into certificate at height 2
    let certificate = RoundCertificate {
        height: Height::new(2),
        round,
        cert_type: RoundCertificateType::Skip,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Prevote,
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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
            expected: 34,
        })
    );
}

/// Signature replay across heights in a PrecommitRound certificate.
#[test]
fn round_precommit_certificate_signature_replay_across_heights() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height_1 = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

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

    let certificate = RoundCertificate {
        height: Height::new(2),
        round,
        cert_type: RoundCertificateType::Precommit,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Precommit,
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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

/// Signature replay across rounds in a SkipRound certificate.
#[test]
fn round_skip_certificate_signature_replay_across_rounds() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round_0 = Round::new(0);
    let value_id = ValueId::new(42);

    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_prevote(
                height,
                round_0,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    let certificate = RoundCertificate {
        height,
        round: Round::new(1),
        cert_type: RoundCertificateType::Skip,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Prevote,
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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
            expected: 34,
        })
    );
}

/// Signature replay across rounds in a PrecommitRound certificate.
#[test]
fn round_precommit_certificate_signature_replay_across_rounds() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round_0 = Round::new(0);
    let value_id = ValueId::new(42);

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

    let certificate = RoundCertificate {
        height,
        round: Round::new(1),
        cert_type: RoundCertificateType::Precommit,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Precommit,
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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

/// Signature replay across values in a SkipRound certificate.
/// The value_id is per-RoundSignature, so the verifier reconstructs the vote
/// with the RoundSignature's value_id. But we put the cert at the same
/// height/round and the signature's value_id matches what was signed, so we
/// need to tamper the value_id in the RoundSignature to simulate a replay.
#[test]
fn round_skip_certificate_signature_replay_across_values() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);

    // Sign valid prevotes for value 42
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(ValueId::new(42)),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject signatures but claim they were for value 99
    let certificate = RoundCertificate {
        height,
        round,
        cert_type: RoundCertificateType::Skip,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Prevote,
                    NilOrVal::Val(ValueId::new(99)),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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
            expected: 34,
        })
    );
}

/// Signature replay across values in a PrecommitRound certificate.
#[test]
fn round_precommit_certificate_signature_replay_across_values() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);

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

    let certificate = RoundCertificate {
        height,
        round,
        cert_type: RoundCertificateType::Precommit,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Precommit,
                    NilOrVal::Val(ValueId::new(99)),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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

/// Cross-type replay in a PrecommitRound certificate: prevote signatures are
/// injected with vote_type set to Prevote. The explicit check for
/// `InvalidVoteType` fires before signature verification.
#[test]
fn round_precommit_certificate_cross_type_replay_from_prevote() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign valid prevotes
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject as Prevote-typed signatures into a Precommit certificate
    let certificate = RoundCertificate {
        height,
        round,
        cert_type: RoundCertificateType::Precommit,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Prevote,
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
        &ctx,
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    ));
    // InvalidVoteType is returned for the first signature before sig verification
    assert!(matches!(
        result,
        Err(CertificateError::InvalidVoteType(_))
    ));
}

/// Cross-type replay in a SkipRound certificate with flipped vote type:
/// sign as precommit but inject as RoundSignature with vote_type=Prevote.
/// The verifier reconstructs a prevote, but the signature was over precommit
/// data, so verification fails.
#[test]
fn round_skip_certificate_cross_type_replay_flipped_vote_type() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    // Sign valid precommits
    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers[i].sign_vote(ctx.new_precommit(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            )))
            .unwrap()
        })
        .collect();

    // Inject with vote_type=Prevote (flipped) into a Skip certificate
    let certificate = RoundCertificate {
        height,
        round,
        cert_type: RoundCertificateType::Skip,
        round_signatures: votes
            .iter()
            .map(|v| {
                RoundSignature::new(
                    VoteType::Prevote, // flipped from Precommit
                    NilOrVal::Val(value_id),
                    v.message.validator_address,
                    v.signature.clone(),
                )
            })
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_round_certificate(
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
            expected: 34,
        })
    );
}

/// Validator set mismatch for SkipRound certificate.
#[test]
fn round_skip_certificate_validator_set_mismatch() {
    let (validators_a, signers_a) = make_validators([25, 25, 25, 25], 0xAAAA);
    let (validators_b, _) = make_validators([25, 25, 25, 25], 0xBBBB);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let votes: Vec<_> = (0..4)
        .map(|i| {
            block_on(signers_a[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators_a[i].address,
            )))
            .unwrap()
        })
        .collect();

    let certificate =
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Skip, votes);

    let validator_set_b = ValidatorSet::new(validators_b.to_vec());
    let result = block_on(signers_a[0].verify_round_certificate(
        &ctx,
        &certificate,
        &validator_set_b,
        ThresholdParams::default(),
    ));
    assert!(matches!(result, Err(CertificateError::UnknownValidator(_))));
}

/// Validator set mismatch for PrecommitRound certificate.
#[test]
fn round_precommit_certificate_validator_set_mismatch() {
    let (validators_a, signers_a) = make_validators([25, 25, 25, 25], 0xAAAA);
    let (validators_b, _) = make_validators([25, 25, 25, 25], 0xBBBB);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

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

    let certificate =
        RoundCertificate::new_from_votes(height, round, RoundCertificateType::Precommit, votes);

    let validator_set_b = ValidatorSet::new(validators_b.to_vec());
    let result = block_on(signers_a[0].verify_round_certificate(
        &ctx,
        &certificate,
        &validator_set_b,
        ThresholdParams::default(),
    ));
    assert!(matches!(result, Err(CertificateError::UnknownValidator(_))));
}

/// Quorum boundary for SkipRound: exactly 1/3 is NOT sufficient (strict >).
/// With validators [1, 1, 1] and 1 of 3 signing: 1*3=3 > 3*1=3 is false.
#[test]
fn round_skip_certificate_quorum_boundary_exact_one_third_insufficient() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([1, 1, 1])
        .with_votes(0..1, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 1,
            total: 3,
            expected: 2,
        });
}

/// Quorum boundary for SkipRound: just above 1/3 is sufficient.
/// With validators [1, 1, 1] and 2 of 3 signing: 2*3=6 > 3*1=3, yes.
#[test]
fn round_skip_certificate_quorum_boundary_just_above_one_third_sufficient() {
    CertificateTest::<RoundSkip>::new()
        .with_validators([1, 1, 1])
        .with_votes(0..2, VoteType::Prevote)
        .expect_valid();
}

/// Quorum boundary for PrecommitRound: exactly 2/3 is NOT sufficient (strict >).
/// With validators [1, 1, 1] and 2 of 3 signing: 2*3=6 > 3*2=6 is false.
#[test]
fn round_precommit_certificate_quorum_boundary_exact_two_thirds_insufficient() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([1, 1, 1])
        .with_votes(0..2, VoteType::Precommit)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 2,
            total: 3,
            expected: 3,
        });
}

/// Quorum boundary for PrecommitRound: just above 2/3 is sufficient.
/// With validators [34, 33, 33], signing [0, 1] (VP=67): 67*3=201 > 100*2=200.
#[test]
fn round_precommit_certificate_quorum_boundary_just_above_two_thirds_sufficient() {
    CertificateTest::<RoundPrecommit>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..3, VoteType::Precommit)
        .expect_valid();

    CertificateTest::<RoundPrecommit>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..2, VoteType::Precommit)
        .expect_valid();
}
