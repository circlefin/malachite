use crate::signing::SigningProvider;
use crate::{
    Address, Height, NilOrVal, Proposal, ProposalPart, Round, SigningScheme, Validator,
    ValidatorSet, Value, ValueId, Vote,
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

    /// The `Value` type denotes the value `v` carried by the `Proposal`
    /// consensus message that is gossiped to other nodes by the proposer.
    type Value: Value;

    /// The type of votes that can be cast.
    type Vote: Vote<Self>;

    /// The signing scheme used to sign consensus messages.
    type SigningScheme: SigningScheme;

    /// The signing provider used to sign and verify consensus messages.
    type SigningProvider: SigningProvider<Self>;

    /// Select a proposer in the validator set for the given height and round.
    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator;

    /// Get the singing provider.
    fn signing_provider(&self) -> &Self::SigningProvider;

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
}
