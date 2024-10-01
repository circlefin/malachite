use std::collections::{BTreeMap, VecDeque};

use malachite_common::*;
use malachite_driver::Driver;

use crate::error::Error;
use crate::input::Input;
use crate::Params;
use crate::ProposedValue;
use crate::{FullProposal, FullProposalKeeper};

/// The state maintained by consensus for processing a [`Input`][crate::Input].
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// The context for the consensus state machine
    pub ctx: Ctx,

    /// Driver for the per-round consensus state machine
    pub driver: Driver<Ctx>,

    /// A queue of inputs that were received before the
    /// driver started the new height and was still at round Nil.
    pub input_queue: VecDeque<Input<Ctx>>,

    /// The proposals to decide on.
    pub full_proposal_keeper: FullProposalKeeper<Ctx>,

    /// Store Precommit votes to be sent along the decision to the host
    pub signed_precommits: BTreeMap<(Ctx::Height, Round), Vec<SignedVote<Ctx>>>,

    /// Decision per height
    pub decision: BTreeMap<(Ctx::Height, Round), Ctx::Proposal>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(ctx: Ctx, params: Params<Ctx>) -> Self {
        let driver = Driver::new(
            ctx.clone(),
            params.start_height,
            params.initial_validator_set,
            params.address,
            params.threshold_params,
        );

        Self {
            ctx,
            driver,
            input_queue: Default::default(),
            full_proposal_keeper: Default::default(),
            signed_precommits: Default::default(),
            decision: Default::default(),
        }
    }

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

    pub fn full_proposal_at_round_and_value(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Option<&FullProposal<Ctx>> {
        self.full_proposal_keeper
            .full_proposal_at_round_and_value(height, round, value)
    }

    pub fn full_proposals_for_value(
        &self,
        proposed_value: &ProposedValue<Ctx>,
    ) -> Vec<SignedProposal<Ctx>> {
        self.full_proposal_keeper
            .full_proposals_for_value(proposed_value)
    }

    pub fn store_proposal(&mut self, new_proposal: SignedProposal<Ctx>) {
        self.full_proposal_keeper.store_proposal(new_proposal)
    }

    pub fn store_value(&mut self, new_value: &ProposedValue<Ctx>) {
        // Values for higher height should have been cached for future processing
        assert_eq!(new_value.height, self.driver.height());
        self.full_proposal_keeper.store_value(new_value)
    }

    pub fn remove_full_proposals(&mut self, height: Ctx::Height) {
        self.full_proposal_keeper.remove_full_proposals(height)
    }
}
