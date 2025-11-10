#![allow(dead_code)]

mod commit;
mod polka;
mod round;

use std::marker::PhantomData;

pub mod types {
    pub use informalsystems_malachitebft_test::{
        utils, Address, Ed25519Provider, Height, TestContext, Validator, ValidatorSet, ValueId,
        Vote,
    };
    pub use malachitebft_core_types::{
        CertificateError, Context, NilOrVal, Round, RoundCertificateType, SignedVote,
        ThresholdParams, VoteType, VotingPower,
    };
    pub use malachitebft_signing::{SigningProvider, SigningProviderExt};
    pub use malachitebft_signing_ed25519::Signature;
}

use futures::executor::block_on;
use types::*;

use rand::Rng;

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

pub trait CertificateBuilder {
    type Certificate;

    fn build_certificate(
        height: Height,
        round: Round,
        value_id: Option<ValueId>,
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
        vote_type: VoteType,
        validator_address: Address,
    ) -> Vote {
        match vote_type {
            VoteType::Prevote => ctx.new_prevote(height, round, value_id, validator_address),
            VoteType::Precommit => ctx.new_precommit(height, round, value_id, validator_address),
        }
    }
}

/// A fluent builder for certificate testing
pub struct CertificateTest<C> {
    ctx: TestContext,
    height: Height,
    round: Round,
    value_id: ValueId,
    validators: Vec<Validator>,
    signers: Vec<Ed25519Provider>,
    votes: Vec<SignedVote<TestContext>>,
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
            votes: Vec::new(),
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

    /// Add votes to include in the certificate
    pub fn with_votes(
        mut self,
        indices: impl IntoIterator<Item = usize>,
        vote_type: VoteType,
    ) -> Self {
        for idx in indices {
            if idx < self.validators.len() {
                let vote = block_on(self.signers[idx].sign_vote(C::make_vote(
                    &self.ctx,
                    self.height,
                    self.round,
                    NilOrVal::Val(self.value_id),
                    vote_type,
                    self.validators[idx].address,
                )))
                .unwrap();

                self.votes.push(vote);
            }
        }
        self
    }

    /// Add nil votes to include in the certificate
    pub fn with_nil_votes(
        mut self,
        indices: impl IntoIterator<Item = usize>,
        vote_type: VoteType,
    ) -> Self {
        for idx in indices {
            if idx < self.validators.len() {
                let vote = block_on(self.signers[idx].sign_vote(C::make_vote(
                    &self.ctx,
                    self.height,
                    self.round,
                    NilOrVal::Nil,
                    vote_type,
                    self.validators[idx].address,
                )))
                .unwrap();

                self.votes.push(vote);
            }
        }
        self
    }

    /// Add a vote with different value to include in the certificate
    pub fn with_different_value_vote(mut self, index: usize, vote_type: VoteType) -> Self {
        if index < self.validators.len() {
            let vote = block_on(self.signers[index].sign_vote(C::make_vote(
                &self.ctx,
                self.height,
                self.round,
                NilOrVal::Val(ValueId::new(85)),
                vote_type,
                self.validators[index].address,
            )))
            .unwrap();

            self.votes.push(vote);
        }
        self
    }

    /// Add votes to include in the certificate with random types and values
    /// If vote_type_opt is Some, uses that vote type; otherwise picks one at random.
    pub fn with_random_votes(
        mut self,
        indices: impl IntoIterator<Item = usize>,
        vote_type_opt: Option<VoteType>,
    ) -> Self {
        let mut rng = rand::thread_rng();

        for idx in indices {
            if idx < self.validators.len() {
                let vote_type = match vote_type_opt {
                    Some(vt) => vt,
                    None => {
                        // Randomly pick vote type
                        if rng.gen_range(0..2) == 0 {
                            VoteType::Prevote
                        } else {
                            VoteType::Precommit
                        }
                    }
                };

                // Randomly pick value kind: 0 = nil, 1 = same value, 2 = different value
                match rng.gen_range(0..3) {
                    0 => self = self.with_nil_votes([idx], vote_type),
                    1 => self = self.with_votes([idx], vote_type),
                    2 => self = self.with_different_value_vote(idx, vote_type),
                    _ => unreachable!(),
                };
            }
        }

        self
    }

    /// Add a vote with invalid height to include in the certificate
    pub fn with_invalid_height_vote(mut self, index: usize, vote_type: VoteType) -> Self {
        if index < self.validators.len() {
            let vote = block_on(self.signers[index].sign_vote(C::make_vote(
                &self.ctx,
                self.height.increment(),
                self.round,
                NilOrVal::Val(self.value_id),
                vote_type,
                self.validators[index].address,
            )))
            .unwrap();

            self.votes.push(vote);
        }
        self
    }

    /// Add a vote with invalid round to include in the certificate
    pub fn with_invalid_round_vote(mut self, index: usize, vote_type: VoteType) -> Self {
        if index < self.validators.len() {
            let vote = block_on(self.signers[index].sign_vote(C::make_vote(
                &self.ctx,
                self.height,
                self.round.increment(),
                NilOrVal::Val(self.value_id),
                vote_type,
                self.validators[index].address,
            )))
            .unwrap();

            self.votes.push(vote);
        }
        self
    }

    /// Add a vote with invalid signature to include in the certificate
    pub fn with_invalid_signature_vote(mut self, index: usize, vote_type: VoteType) -> Self {
        if index < self.validators.len() {
            let mut vote = block_on(self.signers[index].sign_vote(C::make_vote(
                &self.ctx,
                self.height,
                self.round,
                NilOrVal::Val(self.value_id),
                vote_type,
                self.validators[index].address,
            )))
            .unwrap();
            vote.signature = Signature::test(); // Set an invalid signature
            self.votes.push(vote);
        }
        self
    }

    /// Add a vote from external validator to include in the certificate
    pub fn with_non_validator_vote(mut self, seed: u64, vote_type: VoteType) -> Self {
        let ([validator], [signer]) = make_validators([0], seed);
        let vote = block_on(signer.sign_vote(C::make_vote(
            &self.ctx,
            self.height,
            self.round,
            NilOrVal::Val(self.value_id),
            vote_type,
            validator.address,
        )))
        .unwrap();
        self.votes.push(vote);
        self
    }

    /// Add a duplicate last vote to include in the certificate
    pub fn with_duplicate_last_vote(mut self) -> Self {
        if let Some(last_vote) = self.votes.last().cloned() {
            self.votes.push(last_vote);
        }
        self
    }

    /// Build the certificate based on the configured settings
    fn build_certificate(&self) -> (C::Certificate, ValidatorSet) {
        let validator_set = ValidatorSet::new(self.validators.clone());
        let certificate = C::build_certificate(
            self.height,
            self.round,
            Some(self.value_id),
            self.votes.clone(),
        );
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
