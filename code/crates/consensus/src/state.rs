use std::collections::{BTreeMap, VecDeque};

use malachite_common::*;
use malachite_driver::Driver;

use crate::error::Error;
use crate::msg::Msg;

/// The state maintained by consensus for processing a [`Msg`][crate::msg::Msg].
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// The context for the consensus state machine
    pub ctx: Ctx,

    /// Driver for the per-round consensus state machine
    pub driver: Driver<Ctx>,

    /// A queue of gossip events that were received before the
    /// driver started the new height and was still at round Nil.
    pub msg_queue: VecDeque<Msg<Ctx>>,

    /// The value and validity of received blocks.
    pub received_blocks: Vec<(Ctx::Height, Round, Ctx::Value, Validity)>,

    /// Store Precommit votes to be sent along the decision to the host
    pub signed_precommits: BTreeMap<(Ctx::Height, Round), Vec<SignedVote<Ctx>>>,

    /// Decision per height
    pub decision: BTreeMap<(Ctx::Height, Round), Ctx::Proposal>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn get_proposer(
        &self,
        height: Ctx::Height,
        round: Round,
    ) -> Result<&Ctx::Address, Error<Ctx>> {
        assert!(self.driver.validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let proposer_index = {
            let height = height.as_u64() as usize;
            let round = round.as_i64() as usize;

            (height - 1 + round) % self.driver.validator_set.count()
        };

        let proposer = self
            .driver
            .validator_set
            .get_by_index(proposer_index)
            .ok_or(Error::ProposerNotFound(height, round))?;

        Ok(proposer.address())
    }

    pub fn remove_received_block(&mut self, height: Ctx::Height, round: Round) {
        self.received_blocks
            .retain(|&(h, r, ..)| h != height && r != round);
    }

    pub fn store_signed_precommit(&mut self, precommit: SignedVote<Ctx>) {
        assert_eq!(precommit.vote_type(), VoteType::Precommit);

        let height = precommit.height();
        let round = precommit.round();

        self.signed_precommits
            .entry((height, round))
            .or_default()
            .push(precommit);
    }

    pub fn restore_precommits(
        &mut self,
        height: Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Vec<SignedVote<Ctx>> {
        // Get the commits for the height and round.
        let mut commits_for_height_and_round = self
            .signed_precommits
            .remove(&(height, round))
            .unwrap_or_default();

        // Keep the commits for the specified value.
        // For now we ignore equivocating votes if present.
        commits_for_height_and_round.retain(|c| c.value() == &NilOrVal::Val(value.id()));

        commits_for_height_and_round
    }
}
