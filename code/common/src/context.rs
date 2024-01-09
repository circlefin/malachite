use crate::{
    Address, Height, NilOrVal, Proposal, PublicKey, Round, SignedVote, SigningScheme, Validator,
    ValidatorSet, Value, ValueId, Vote,
};

/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Context
where
    Self: Sized,
{
    /// The type of address of a validator.
    type Address: Address;

    /// The type of the height of a block.
    type Height: Height;

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

    /// Sign the given vote our private key.
    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self>;

    /// Verify the given vote's signature using the given public key.
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
}
