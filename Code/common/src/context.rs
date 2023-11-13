use crate::{
    Address, Height, Proposal, PublicKey, Round, SignedVote, SigningScheme, Validator,
    ValidatorSet, Value, ValueId, Vote,
};

/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Context
where
    Self: Sized,
{
    type Address: Address;
    type Height: Height;
    type Proposal: Proposal<Self>;
    type Validator: Validator<Self>;
    type ValidatorSet: ValidatorSet<Self>;
    type Value: Value;
    type Vote: Vote<Self>;
    type SigningScheme: SigningScheme; // TODO: Do we need to support multiple signing schemes?

    // FIXME: Remove altogether
    const DUMMY_VALUE: Self::Value;

    /// Sign the given vote our private key.
    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self>;

    /// Verify the given vote's signature using the given public key.
    /// TODO: Maybe move this as concrete methods in `SignedVote`?
    fn verify_signed_vote(
        &self,
        signed_vote: &SignedVote<Self>,
        public_key: &PublicKey<Self>,
    ) -> bool;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(
        round: Round,
        value_id: Option<ValueId<Self>>,
        address: Self::Address,
    ) -> Self::Vote;
}
