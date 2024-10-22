use crate::{
    Address, Extension, Height, NilOrVal, Proposal, ProposalPart, PublicKey, Round, Signature,
    SignedMessage, SigningScheme, Validator, ValidatorSet, Value, ValueId, Vote,
};

/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Context
where
    Self: Sized + Clone + Send + Sync + 'static,
{
    /// The type of address of a validator.
    type Address: Address;

    /// The type of the height of a block.
    type Height: Height;

    /// The type of proposal part
    type ProposalPart: ProposalPart<Self>;

    /// The interface provided by the proposal type.
    type Proposal: Proposal<Self>;

    /// The interface provided by the validator type.
    type Validator: Validator<Self>;

    /// The interface provided by the validator set type.
    type ValidatorSet: ValidatorSet<Self>;

    /// The type of values that can be proposed.
    type Value: Value;

    /// The type of votes that can be cast.
    type Vote: Vote<Self>;

    /// The signing scheme used to sign votes.
    type SigningScheme: SigningScheme;

    /// Select a proposer in the validator set for the given height and round.
    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator;

    /// Sign the given vote with our private key.
    fn sign_vote(&self, vote: Self::Vote) -> SignedMessage<Self, Self::Vote>;

    /// Verify the given vote's signature using the given public key.
    fn verify_signed_vote(
        &self,
        vote: &Self::Vote,
        signature: &Signature<Self>,
        public_key: &PublicKey<Self>,
    ) -> bool;

    /// Sign the given proposal with our private key.
    fn sign_proposal(&self, proposal: Self::Proposal) -> SignedMessage<Self, Self::Proposal>;

    /// Verify the given proposal's signature using the given public key.
    fn verify_signed_proposal(
        &self,
        proposal: &Self::Proposal,
        signature: &Signature<Self>,
        public_key: &PublicKey<Self>,
    ) -> bool;

    /// Sign the proposal part with our private key.
    fn sign_proposal_part(
        &self,
        proposal_part: Self::ProposalPart,
    ) -> SignedMessage<Self, Self::ProposalPart>;

    /// Verify the given proposal part signature using the given public key.
    fn verify_signed_proposal_part(
        &self,
        proposal_part: &Self::ProposalPart,
        signature: &Signature<Self>,
        public_key: &PublicKey<Self>,
    ) -> bool;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
        address: Self::Address,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(
        height: Self::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(
        height: Self::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit that includes an extension
    fn extended_precommit(
        height: Self::Height,
        round: Round,
        value_id: NilOrVal<ValueId<Self>>,
        address: Self::Address,
        _extension: Extension,
    ) -> Self::Vote {
        Self::new_precommit(height, round, value_id, address)
    }
}
