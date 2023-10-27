use crate::{
    Address, Height, PrivateKey, Proposal, PublicKey, Round, Signature, SignedVote, Validator,
    ValidatorSet, Value, ValueId, Vote,
};

/// This trait allows to abstract over the various datatypes
/// that are used in the consensus engine.
pub trait Consensus
where
    Self: Sized,
{
    type Address: Address;
    type Height: Height;
    type Proposal: Proposal<Self>;
    type PrivateKey: PrivateKey<PublicKey = Self::PublicKey>;
    type PublicKey: PublicKey<Signature = Signature<Self>>;
    type Validator: Validator<Self>;
    type ValidatorSet: ValidatorSet<Self>;
    type Value: Value;
    type Vote: Vote<Self>;

    // FIXME: Remove altogether
    const DUMMY_VALUE: Self::Value;

    fn sign_vote(vote: &Self::Vote, private_key: &Self::PrivateKey) -> Signature<Self>;

    fn verify_signed_vote(signed_vote: &SignedVote<Self>, public_key: &Self::PublicKey) -> bool;

    /// Build a new proposal for the given value at the given height, round and POL round.
    fn new_proposal(
        height: Self::Height,
        round: Round,
        value: Self::Value,
        pol_round: Round,
    ) -> Self::Proposal;

    /// Build a new prevote vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_prevote(round: Round, value_id: Option<ValueId<Self>>) -> Self::Vote;

    /// Build a new precommit vote by the validator with the given address,
    /// for the value identified by the given value id, at the given round.
    fn new_precommit(round: Round, value_id: Option<ValueId<Self>>) -> Self::Vote;
}
