use informalsystems_malachitebft_test::{
    utils, Ed25519Provider, Height, TestContext, Validator, ValidatorSet, ValueId,
};
use malachitebft_core_types::{
    CertificateError, Context, NilOrVal, PolkaCertificate, Round, SigningProvider,
    SigningProviderExt, ThresholdParams, VotingPower,
};
use malachitebft_signing_ed25519::Signature;

pub fn make_validators<const N: usize>(
    voting_powers: [VotingPower; N],
    seed: u64,
) -> ([Validator; N], [Ed25519Provider; N]) {
    let (validators, private_keys): (Vec<_>, Vec<_>) =
        utils::validators::make_validators_seeded(voting_powers, seed)
            .into_iter()
            .map(|(v, pk)| (v, Ed25519Provider::new(pk)))
            .unzip();

    (
        validators.try_into().unwrap(),
        private_keys.try_into().unwrap(),
    )
}

fn valid_certificate<const M: usize, const N: usize>(voting_powers: [VotingPower; N]) {
    assert!(M <= N);

    let ctx = TestContext::new();

    let (validators, signers) = make_validators(voting_powers, 42);
    let validator_set = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let votes = (0..M)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            ))
        })
        .collect();

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert!(
            result.is_ok(),
            "Expected valid certificate, but got error: {:?}",
            result.unwrap_err()
        );
    }
}

/// Tests the verification of a valid PolkaCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_polka_certificate_with_sufficient_voting_power() {
    valid_certificate::<4, 4>([20, 20, 30, 30]);
    valid_certificate::<3, 4>([20, 20, 30, 30]);
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
///
/// Certificate: Contains valid signatures from validators representing exactly 2/3+ of total voting power
/// Validator Set: 4 validators
#[test]
fn valid_polka_certificate_with_exact_threshold_voting_power() {
    valid_certificate::<3, 4>([21, 22, 24, 30]);
    valid_certificate::<3, 4>([21, 22, 24, 0]);
}

fn invalid_certificate<const M: usize, const N: usize>(
    voting_powers: [VotingPower; N],
    make_value: impl Fn(usize, ValueId) -> NilOrVal<ValueId>,
    error: CertificateError<TestContext>,
) {
    assert!(M <= N);

    let ctx = TestContext::new();

    let (validators, signers) = make_validators(voting_powers, 42);
    let validator_set = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let votes = (0..M)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                make_value(i, value_id),
                validators[i].address,
            ))
        })
        .collect();

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert_eq!(
            result.as_ref(),
            Err(&error),
            "Expected invalid certificate, but got: {result:?}",
        );
    }
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
///
/// Certificate: Contains valid signatures but only from validators representing 65% of total voting power
/// Validator Set: Multiple validators with varying voting powers
/// Expected: CertificateError::NotEnoughVotingPower
#[test]
fn invalid_polka_certificate_insufficient_voting_power() {
    invalid_certificate::<3, 4>(
        [10, 20, 30, 40],
        |_, v| NilOrVal::Val(v),
        CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        },
    );

    invalid_certificate::<2, 4>(
        [10, 10, 30, 50],
        |_, v| NilOrVal::Val(v),
        CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        },
    );

    invalid_certificate::<4, 4>(
        [10, 10, 30, 50],
        |_, _| NilOrVal::Nil,
        CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 100,
            expected: 67,
        },
    );
}

/// Tests the verification of a certificate containing multiple votes from the same validator.
///
/// Certificate: Contains 2 votes from the same validator (duplicate vote)
/// Validator Set: Multiple valid validators
/// Expected: CertificateError::DuplicateVote
#[test]
fn invalid_polka_certificate_duplicate_validator_vote() {
    let ctx = TestContext::new();

    let (validators, signers) = make_validators([10, 10, 10, 10], 42);
    let validator_set = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let mut votes = (0..4)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            ))
        })
        .collect::<Vec<_>>();

    votes.push(signers[0].sign_vote(ctx.new_prevote(
        height,
        round,
        NilOrVal::Val(value_id),
        validators[0].address,
    )));

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert_eq!(
            result,
            Err(CertificateError::DuplicateVote {
                address: validators[0].address,
            }),
            "Expected invalid certificate, but got: {result:?}",
        );
    }
}

/// Tests the verification of a certificate containing a vote from a validator not in the validator set.
///
/// Certificate: Contains a vote from an address not present in the validator set
/// Validator Set: Does not include the validator address in question
/// Expected: CertificateError::UnknownValidator
#[test]
fn invalid_polka_certificate_unknown_validator() {
    let ctx = TestContext::new();

    let (validators, signers) = make_validators([10, 10, 10, 10], 1);
    let validator_set = ValidatorSet::new(validators.clone());
    let ([other_validator], [other_signer]) = make_validators([0], 2);

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let mut votes = (0..4)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            ))
        })
        .collect::<Vec<_>>();

    // Add a vote from an unknown validator
    votes.push(other_signer.sign_vote(ctx.new_prevote(
        height,
        round,
        NilOrVal::Val(value_id),
        other_validator.address,
    )));

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert_eq!(
            result,
            Err(CertificateError::UnknownValidator(other_validator.address,)),
            "Expected invalid certificate, but got: {result:?}",
        );
    }
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
///
/// Certificate: Contains a vote where the signature doesn't match the message
/// Expected: The vote should not contribute to the signed_voting_power
#[test]
fn invalid_polka_certificate_invalid_signature_1() {
    let ctx = TestContext::new();

    let (validators, signers) = make_validators([10, 10, 10], 42);
    let validator_set = ValidatorSet::new(validators.clone());

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let mut votes = (0..3)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            ))
        })
        .collect::<Vec<_>>();

    // Replace the signature with an invalid one
    votes[0].signature = Signature::test();

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert_eq!(
            result,
            Err(CertificateError::NotEnoughVotingPower {
                signed: 20,
                total: 30,
                expected: 21
            }),
            "Expected invalid certificate, but got: {result:?}",
        );
    }
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
///
/// Certificate: Contains a vote where the signature doesn't match the public key
/// Expected: The vote should not contribute to the signed_voting_power
#[test]
fn invalid_polka_certificate_invalid_signature_2() {
    let ctx = TestContext::new();

    let (validators, signers) = make_validators([10, 10, 10], 1);
    let validator_set = ValidatorSet::new(validators.clone());
    let (_, [other_signer]) = make_validators([0], 2);

    let height = Height::new(1);
    let round = Round::new(0);
    let value_id = ValueId::new(42);

    let mut votes = (0..3)
        .map(|i| {
            signers[i].sign_vote(ctx.new_prevote(
                height,
                round,
                NilOrVal::Val(value_id),
                validators[i].address,
            ))
        })
        .collect::<Vec<_>>();

    // Replace the signature with a signature from another signer
    votes[0].signature = other_signer.sign_vote(votes[0].message.clone()).signature;

    let certificate = PolkaCertificate {
        height,
        round,
        value_id,
        votes,
    };

    for signer in signers {
        let result = signer.verify_polka_certificate(
            &certificate,
            &validator_set,
            ThresholdParams::default(),
        );

        assert_eq!(
            result,
            Err(CertificateError::NotEnoughVotingPower {
                signed: 20,
                total: 30,
                expected: 21
            }),
            "Expected invalid certificate, but got: {result:?}",
        );
    }
}

/// Tests the verification of a certificate with no votes.
///
/// Certificate: Empty list of votes
/// Validator Set: Any non-empty validator set
/// Expected: CertificateError::NotEnoughVotingPower
#[test]
fn empty_polka_certificate() {
    let certificate = PolkaCertificate::<TestContext> {
        height: Height::new(1),
        round: Round::new(0),
        value_id: ValueId::new(42),
        votes: vec![],
    };

    let (validators, signers) = make_validators([1, 1, 1], 42);
    let validator_set = ValidatorSet::new(validators);

    let result = signers[0].verify_polka_certificate(
        &certificate,
        &validator_set,
        ThresholdParams::default(),
    );

    assert_eq!(
        result.unwrap_err(),
        CertificateError::NotEnoughVotingPower {
            signed: 0,
            total: 3,
            expected: 3
        }
    );
}
