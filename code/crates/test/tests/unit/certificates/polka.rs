#![allow(dead_code)]

use informalsystems_malachitebft_test::{
    utils, Ed25519Provider, Height, TestContext, Validator, ValidatorSet, ValueId,
};
use malachitebft_core_types::{
    CertificateError, Context, NilOrVal, PolkaCertificate, Round, SignedVote, SigningProvider,
    SigningProviderExt, ThresholdParams, VotingPower,
};
use malachitebft_signing_ed25519::Signature;

const DEFAULT_SEED: u64 = 0xfeedbeef;

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

enum VoteSpec {
    Normal {
        validator_idx: usize,
        is_nil: bool,
        invalid_signature: bool,
    },
    Duplicate {
        validator_idx: usize,
    },
}

/// A fluent builder for certificate testing
pub struct CertificateTest {
    ctx: TestContext,
    height: Height,
    round: Round,
    value_id: ValueId,
    validators: Vec<Validator>,
    signers: Vec<Ed25519Provider>,
    vote_specs: Vec<VoteSpec>,
    external_votes: Vec<SignedVote<TestContext>>,
}

impl CertificateTest {
    /// Create a new certificate test with default settings
    pub fn new() -> Self {
        Self {
            ctx: TestContext::new(),
            height: Height::new(1),
            round: Round::new(0),
            value_id: ValueId::new(42),
            validators: Vec::new(),
            signers: Vec::new(),
            vote_specs: Vec::new(),
            external_votes: Vec::new(),
        }
    }

    /// Set the height for the certificate
    pub fn with_height(mut self, height: u64) -> Self {
        self.height = Height::new(height);
        self
    }

    /// Set the round for the certificate
    pub fn with_round(mut self, round: i64) -> Self {
        self.round = Round::from(round);
        self
    }

    /// Set the value ID for the certificate
    pub fn for_value(mut self, value_id: u64) -> Self {
        self.value_id = ValueId::new(value_id);
        self
    }

    /// Set up validators with the given voting powers using default seed
    pub fn with_validators<const N: usize>(self, voting_powers: [VotingPower; N]) -> Self {
        self.with_validators_seeded(voting_powers, DEFAULT_SEED)
    }

    /// Set up validators with the given voting powers and seed
    pub fn with_validators_seeded<const N: usize>(
        mut self,
        voting_powers: [VotingPower; N],
        seed: u64,
    ) -> Self {
        let (validators, signers) = make_validators(voting_powers, seed);
        self.validators = Vec::from(validators);
        self.signers = Vec::from(signers);
        self
    }

    /// Specify which validators should sign the certificate
    pub fn with_signatures(mut self, indices: impl IntoIterator<Item = usize>) -> Self {
        for idx in indices {
            if idx < self.validators.len() {
                self.vote_specs.push(VoteSpec::Normal {
                    validator_idx: idx,
                    is_nil: false,
                    invalid_signature: false,
                });
            }
        }
        self
    }

    /// Add a duplicate vote from the specified validator index
    pub fn with_duplicate_vote(mut self, index: usize) -> Self {
        if index < self.validators.len() {
            self.vote_specs.push(VoteSpec::Duplicate {
                validator_idx: index,
            });
        }
        self
    }

    /// Make all validators vote for nil instead of the value
    pub fn all_vote_nil(mut self) -> Self {
        for spec in &mut self.vote_specs {
            if let VoteSpec::Normal { is_nil, .. } = spec {
                *is_nil = true;
            }
        }
        self
    }

    /// Specify that a validator's signature should be invalid
    pub fn with_invalid_signature(mut self, index: usize) -> Self {
        for spec in &mut self.vote_specs {
            if let VoteSpec::Normal {
                validator_idx,
                invalid_signature,
                ..
            } = spec
            {
                if *validator_idx == index {
                    *invalid_signature = true;
                }
            }
        }
        self
    }

    /// Add a vote from an external validator
    pub fn with_external_vote(mut self, seed: u64) -> Self {
        let ([validator], [signer]) = make_validators([0], seed);
        let vote = signer.sign_vote(self.ctx.new_prevote(
            self.height,
            self.round,
            NilOrVal::Val(self.value_id),
            validator.address,
        ));
        self.external_votes.push(vote);
        self
    }

    /// Build the certificate based on the configured settings
    fn build_certificate(&self) -> (PolkaCertificate<TestContext>, ValidatorSet) {
        let validator_set = ValidatorSet::new(self.validators.clone());

        let mut votes = Vec::new();

        // Process each vote specification
        for spec in &self.vote_specs {
            match spec {
                VoteSpec::Normal {
                    validator_idx,
                    is_nil,
                    invalid_signature,
                } => {
                    let value = if *is_nil {
                        NilOrVal::Nil
                    } else {
                        NilOrVal::Val(self.value_id)
                    };

                    let mut vote = self.signers[*validator_idx].sign_vote(self.ctx.new_prevote(
                        self.height,
                        self.round,
                        value,
                        self.validators[*validator_idx].address,
                    ));

                    if *invalid_signature {
                        vote.signature = Signature::test();
                    }

                    votes.push(vote);
                }
                VoteSpec::Duplicate { validator_idx } => {
                    // For a duplicate, we just create another vote from the same validator
                    let vote = self.signers[*validator_idx].sign_vote(self.ctx.new_prevote(
                        self.height,
                        self.round,
                        NilOrVal::Val(self.value_id),
                        self.validators[*validator_idx].address,
                    ));

                    votes.push(vote);
                }
            }
        }

        // Add external votes
        votes.extend(self.external_votes.clone());

        let certificate = PolkaCertificate {
            height: self.height,
            round: self.round,
            value_id: self.value_id,
            votes,
        };

        (certificate, validator_set)
    }

    /// Verify that the certificate is valid
    pub fn expect_valid(self) {
        let (certificate, validator_set) = self.build_certificate();

        for signer in &self.signers {
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

    /// Verify that the certificate is invalid with the expected error
    pub fn expect_error(self, expected_error: CertificateError<TestContext>) {
        let (certificate, validator_set) = self.build_certificate();

        for signer in &self.signers {
            let result = signer.verify_polka_certificate(
                &certificate,
                &validator_set,
                ThresholdParams::default(),
            );

            assert_eq!(
                result.as_ref(),
                Err(&expected_error),
                "Expected certificate error {expected_error:?}, but got: {result:?}",
            );
        }
    }
}

/// Tests the verification of a valid PolkaCertificate with signatures from validators
/// representing more than 2/3 of the total voting power.
#[test]
fn valid_polka_certificate_with_sufficient_voting_power() {
    CertificateTest::new()
        .with_validators([20, 20, 30, 30])
        .with_signatures(0..4)
        .expect_valid();

    CertificateTest::new()
        .with_validators([20, 20, 30, 30])
        .with_signatures(0..3)
        .expect_valid();
}

/// Tests the verification of a certificate with signatures from validators
/// representing exactly the threshold amount of voting power.
#[test]
fn valid_polka_certificate_with_exact_threshold_voting_power() {
    CertificateTest::new()
        .with_validators([21, 22, 24, 30])
        .with_signatures(0..3)
        .expect_valid();

    CertificateTest::new()
        .with_validators([21, 22, 24, 0])
        .with_signatures(0..3)
        .expect_valid();
}

/// Tests the verification of a certificate with valid signatures but insufficient voting power.
#[test]
fn invalid_polka_certificate_insufficient_voting_power() {
    CertificateTest::new()
        .with_validators([10, 20, 30, 40])
        .with_signatures(0..3)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 60,
            total: 100,
            expected: 67,
        });

    CertificateTest::new()
        .with_validators([10, 10, 30, 50])
        .with_signatures(0..2)
        .expect_error(CertificateError::NotEnoughVotingPower {
            signed: 20,
            total: 100,
            expected: 67,
        });

    CertificateTest::new()
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
fn invalid_polka_certificate_duplicate_validator_vote() {
    let validator_addr = {
        let (validators, _) = make_validators([10, 10, 10, 10], DEFAULT_SEED);
        validators[0].address
    };

    CertificateTest::new()
        .with_validators([10, 10, 10, 10])
        .with_signatures(0..4)
        .with_duplicate_vote(0) // Add duplicate vote from validator 0
        .expect_error(CertificateError::DuplicateVote {
            address: validator_addr,
        });
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

    CertificateTest::new()
        .with_validators([10, 10, 10, 10])
        .with_signatures(0..4)
        .with_external_vote(seed)
        .expect_error(CertificateError::UnknownValidator(external_validator_addr));
}

/// Tests the verification of a certificate containing a vote with an invalid signature.
#[test]
fn invalid_polka_certificate_invalid_signature_1() {
    CertificateTest::new()
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
fn invalid_polka_certificate_invalid_signature_2() {
    CertificateTest::new()
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
fn empty_polka_certificate() {
    CertificateTest::new()
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
fn polka_certificate_with_mixed_valid_and_invalid_votes() {
    CertificateTest::new()
        .with_validators([10, 20, 30, 40])
        .with_signatures(0..4)
        .with_invalid_signature(0) // Invalid signature for validator 0
        .with_invalid_signature(1) // Invalid signature for validator 1
        .expect_valid();

    CertificateTest::new()
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
