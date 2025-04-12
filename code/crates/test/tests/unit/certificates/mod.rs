#![allow(dead_code)]

mod commit;
mod polka;

use std::marker::PhantomData;

pub mod types {
    pub use informalsystems_malachitebft_test::{
        utils, Address, Ed25519Provider, Height, TestContext, Validator, ValidatorSet, ValueId,
        Vote,
    };
    pub use malachitebft_core_types::{
        CertificateError, Context, NilOrVal, Round, SignedVote, SigningProvider,
        SigningProviderExt, ThresholdParams, VotingPower,
    };
    pub use malachitebft_signing_ed25519::Signature;
}

use types::*;

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

pub enum VoteSpec {
    Normal {
        validator_idx: usize,
        is_nil: bool,
        invalid_signature: bool,
        invalid_height: bool,
        invalid_round: bool,
    },
    Duplicate {
        validator_idx: usize,
    },
}

pub trait CertificateBuilder {
    type Certificate;

    fn build_certificate(
        height: Height,
        round: Round,
        value_id: ValueId,
        votes: Vec<SignedVote<TestContext>>,
    ) -> Self::Certificate;

    fn verify_certificate(
        ctx: &TestContext,
        signer: &Ed25519Provider,
        certificate: &Self::Certificate,
        validator_set: &ValidatorSet,
        threshold_params: ThresholdParams,
    ) -> Result<(), CertificateError<TestContext>>;

    fn make_vote(
        ctx: &TestContext,
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        validator_address: Address,
    ) -> Vote;
}

/// A fluent builder for certificate testing
pub struct CertificateTest<C> {
    ctx: TestContext,
    height: Height,
    round: Round,
    value_id: ValueId,
    validators: Vec<Validator>,
    signers: Vec<Ed25519Provider>,
    vote_specs: Vec<VoteSpec>,
    external_votes: Vec<SignedVote<TestContext>>,
    marker: PhantomData<C>,
}

impl<C> CertificateTest<C>
where
    C: CertificateBuilder,
{
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
            marker: PhantomData,
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
                    invalid_height: false,
                    invalid_round: false,
                });
            }
        }
        self
    }

    pub fn with_invalid_vote_height(mut self, index: usize) -> Self {
        if index < self.validators.len() {
            self.vote_specs.push(VoteSpec::Normal {
                validator_idx: index,
                is_nil: false,
                invalid_signature: false,
                invalid_height: true,
                invalid_round: false,
            });
        }
        self
    }

    pub fn with_invalid_vote_round(mut self, index: usize) -> Self {
        if index < self.validators.len() {
            self.vote_specs.push(VoteSpec::Normal {
                validator_idx: index,
                is_nil: false,
                invalid_signature: false,
                invalid_height: false,
                invalid_round: true,
            });
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
        let vote = signer.sign_vote(C::make_vote(
            &self.ctx,
            self.height,
            self.round,
            NilOrVal::Val(self.value_id),
            validator.address,
        ));
        self.external_votes.push(vote);
        self
    }

    /// Build the certificate based on the configured settings
    fn build_certificate(&self) -> (C::Certificate, ValidatorSet) {
        let validator_set = ValidatorSet::new(self.validators.clone());

        let mut votes = Vec::new();

        // Process each vote specification
        for spec in &self.vote_specs {
            match spec {
                VoteSpec::Normal {
                    validator_idx,
                    is_nil,
                    invalid_signature,
                    invalid_height,
                    invalid_round,
                } => {
                    let value = if *is_nil {
                        NilOrVal::Nil
                    } else {
                        NilOrVal::Val(self.value_id)
                    };

                    let height = if *invalid_height {
                        self.height.increment()
                    } else {
                        self.height
                    };

                    let round = if *invalid_round {
                        self.round.increment()
                    } else {
                        self.round
                    };

                    let mut vote = self.signers[*validator_idx].sign_vote(C::make_vote(
                        &self.ctx,
                        height,
                        round,
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
                    let vote = self.signers[*validator_idx].sign_vote(C::make_vote(
                        &self.ctx,
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

        let certificate = C::build_certificate(self.height, self.round, self.value_id, votes);
        (certificate, validator_set)
    }

    /// Verify that the certificate is valid
    pub fn expect_valid(self) {
        let (certificate, validator_set) = self.build_certificate();

        for signer in &self.signers {
            let result = C::verify_certificate(
                &self.ctx,
                signer,
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
            let result = C::verify_certificate(
                &self.ctx,
                signer,
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
