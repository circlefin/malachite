use malachitebft_core_types::{Context, Round};

use crate::{Address, Height, TestContext, ValidatorSet};

/// Defines how to select a proposer amongst a validator set for a given round.
pub trait ProposerSelector<Ctx>
where
    Self: Send + Sync,
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
    fn select_proposer(
        &self,
        height: Ctx::Height,
        round: Round,
        validator_set: &Ctx::ValidatorSet,
    ) -> Ctx::Address;
}

#[derive(Copy, Clone, Debug, Default)]
pub struct RotateProposer;

impl ProposerSelector<TestContext> for RotateProposer {
    fn select_proposer(
        &self,
        height: Height,
        round: Round,
        validator_set: &ValidatorSet,
    ) -> Address {
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let height = height.as_u64() as usize;
        let round = round.as_i64() as usize;

        let proposer_index = (height - 1 + round) % validator_set.validators.len();
        validator_set.validators[proposer_index].address
    }
}

#[derive(Copy, Clone, Debug)]
pub struct FixedProposer {
    proposer: Address,
}

impl FixedProposer {
    pub fn new(proposer: Address) -> Self {
        Self { proposer }
    }
}

impl ProposerSelector<TestContext> for FixedProposer {
    fn select_proposer(
        &self,
        _height: Height,
        _round: Round,
        _validator_set: &ValidatorSet,
    ) -> Address {
        self.proposer
    }
}
