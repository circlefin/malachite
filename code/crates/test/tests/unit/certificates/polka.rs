use futures::executor::block_on;
use malachitebft_core_types::PolkaCertificate;
use malachitebft_signing::SigningProviderExt;

use super::{make_validators, types::*, CertificateBuilder, CertificateTest, DEFAULT_SEED};

pub struct Polka;

impl CertificateBuilder for Polka {
    type Certificate = PolkaCertificate<TestContext>;

    fn build_certificate(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate {
        let value_id = value_id.expect("value_id must be Some(_) in polka certificate");
        PolkaCertificate::new(height, round, value_id, votes)
    }

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>> {
        block_on(signer.verify_polka_certificate(ctx, certificate, validator_set, threshold_params))
    }
}

/// Tests the verification of a valid PolkaCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_polka_certificate_with_sufficient_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..4, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<Polka>::new()
        .with_validators([20, 20, 30, 30])
        .with_votes(0..3, VoteType::Prevote)
        .expect_valid();
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_polka_certificate_with_exact_threshold_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([21, 22, 24, 30])
        .with_votes(0..3, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<Polka>::new()
        .with_validators([21, 22, 24, 0])
        .with_votes(0..3, VoteType::Prevote)
        .expect_valid();
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_polka_certificate_insufficient_voting_power() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..3, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 30, 50])
        .with_votes(0..2, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        });

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 30, 50])
        .with_nil_votes(0..4, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        });
}

/// Tests the verification of a certificate containing multiple votes from the same validator.
#[test]
fn invalid_polka_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[2].address
    };

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .with_duplicate_last_vote() // Add duplicate vote from validator 2
        .expect_error(CertificateError::DuplicateVote(validator_addr));
}

/// Tests the verification of a certificate containing a vote from a validator not in the validator set.
#[test]
fn invalid_polka_certificate_unknown_validator() {
    // Define the seed for generating the other validator twice
    let seed = 0xcafecafe;

    let external_validator_addr = {
        let ([validator], _) = make_validators([0], seed);
        validator.address
    };

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10, 10])
        .with_votes(0..3, VoteType::Prevote)
        .with_non_validator_vote(seed, VoteType::Prevote)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[test]
fn invalid_polka_certificate_invalid_signature() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_signature_vote(2, VoteType::Prevote) // Validator 2 has invalid signature
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate containing a vote with invalid height or round.
#[test]
fn invalid_polka_certificate_wrong_vote_height_round() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_height_vote(2, VoteType::Prevote) // Validator 2 has invalid vote height
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });

    CertificateTest::<Polka>::new()
        .with_validators([10, 10, 10])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_round_vote(2, VoteType::Prevote) // Validator 2 has invalid vote round
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 30,
            expected: 21,
        });
}

/// Tests the verification of a certificate with no votes.
#[test]
fn empty_polka_certificate() {
    CertificateTest::<Polka>::new()
        .with_validators([1, 1, 1])
        .with_votes([], VoteType::Prevote) // No signatures
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 3,
        });
}

/// Tests the verification of a certificate containing both valid and invalid votes.
#[test]
fn polka_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(2..4, VoteType::Prevote)
        .with_invalid_signature_vote(0, VoteType::Prevote) // Invalid signature for validator 0
        .with_invalid_signature_vote(1, VoteType::Prevote) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::<Polka>::new()
        .with_validators([10, 20, 30, 40])
        .with_votes(0..2, VoteType::Prevote)
        .with_invalid_signature_vote(2, VoteType::Prevote) // Invalid signature for validator 2
        .with_invalid_signature_vote(3, VoteType::Prevote) // Invalid signature for validator 3
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

/// Address spoofing attack: a spoofed signature claims to be from a high-VP validator
/// but was actually signed by a different validator's key.
#[test]
fn polka_certificate_address_spoofing_attack() {
    CertificateTest::<Polka>::new()
        .with_validators([10, 90])
        .with_spoofed_address_vote(1, 0, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        });
}

/// Signature replay across heights: valid prevote signatures from height 1
/// are injected into a polka certificate at height 2.
#[test]
fn polka_certificate_signature_replay_across_heights() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height_1 = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

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

    let certificate = PolkaCertificate {
        height: Height::new(2),
        round,
        value_id,
        polka_signatures: votes
            .iter()
            .map(|v| PolkaSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_polka_certificate(
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

/// Signature replay across rounds: valid prevote signatures from round 0
/// are injected into a polka certificate at round 1.
#[test]
fn polka_certificate_signature_replay_across_rounds() {
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

    let certificate = PolkaCertificate {
        height,
        round: Round::new(1),
        value_id,
        polka_signatures: votes
            .iter()
            .map(|v| PolkaSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_polka_certificate(
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

/// Signature replay across values: valid prevote signatures for value 42
/// are injected into a polka certificate for value 99.
#[test]
fn polka_certificate_signature_replay_across_values() {
    let (validators, signers) = make_validators([25, 25, 25, 25], DEFAULT_SEED);
    let ctx = TestContext::new();
    let height = Height::new(1);
    let round = Round::new(0);

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

    let certificate = PolkaCertificate {
        height,
        round,
        value_id: ValueId::new(99),
        polka_signatures: votes
            .iter()
            .map(|v| PolkaSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_polka_certificate(
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

/// Cross-type replay: valid precommit signatures are injected into a polka
/// (prevote) certificate. The verifier reconstructs prevotes, but the signatures
/// were over precommit data, so verification fails.
#[test]
fn polka_certificate_cross_type_replay_from_precommit() {
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

    // Inject precommit signatures into a polka (prevote) certificate
    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        polka_signatures: votes
            .iter()
            .map(|v| PolkaSignature::new(v.message.validator_address, v.signature.clone()))
            .collect(),
    };

    let validator_set = ValidatorSet::new(validators.to_vec());
    let result = block_on(signers[0].verify_polka_certificate(
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
fn polka_certificate_validator_set_mismatch() {
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

    let certificate = PolkaCertificate::new(height, round, value_id, votes);

    let validator_set_b = ValidatorSet::new(validators_b.to_vec());
    let result = block_on(signers_a[0].verify_polka_certificate(
        &ctx,
        &certificate,
        &validator_set_b,
        ThresholdParams::default(),
    ));
    assert!(matches!(result, Err(CertificateError::UnknownValidator(_))));
}

/// Quorum boundary: exactly 2/3 is NOT sufficient (strict >).
/// With validators [1, 1, 1] and 2 of 3 signing: 2*3=6 > 3*2=6 is false.
#[test]
fn polka_certificate_quorum_boundary_exact_two_thirds_insufficient() {
    CertificateTest::<Polka>::new()
        .with_validators([1, 1, 1])
        .with_votes(0..2, VoteType::Prevote)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 2,
            total: 3,
            expected: 3,
        });
}

/// Quorum boundary: just above 2/3 is sufficient.
/// With validators [34, 33, 33], signing [0, 1] (VP=67): 67*3=201 > 100*2=200.
#[test]
fn polka_certificate_quorum_boundary_just_above_two_thirds_sufficient() {
    CertificateTest::<Polka>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..3, VoteType::Prevote)
        .expect_valid();

    CertificateTest::<Polka>::new()
        .with_validators([34, 33, 33])
        .with_votes(0..2, VoteType::Prevote)
        .expect_valid();
}
