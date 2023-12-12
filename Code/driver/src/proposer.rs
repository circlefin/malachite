use malachite_common::{Context, Round};

/// Defines how to select a proposer amongst a validator set for a given round.
pub trait ProposerSelector<Ctx>
where
    Ctx: Context,
{
    /// Select a proposer from the given validator set for the given round.
    ///
    /// This function is called at the beginning of each round to select the proposer for that
    /// round. The proposer is responsible for proposing a value for the round.
    ///
    /// # Important
    /// This function must be deterministic!
    /// For a given round and validator set, it must always return the same proposer.
    fn select_proposer(&self, round: Round, validator_set: &Ctx::ValidatorSet) -> Ctx::Address;
}
