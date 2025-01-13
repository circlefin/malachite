use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, warn};

use malachitebft_core_driver::Driver;
use malachitebft_core_types::*;

use crate::input::Input;
use crate::util::max_queue::MaxQueue;
use crate::{FullProposal, FullProposalKeeper, Params, ProposedValue};

/// The state maintained by consensus for processing a [`Input`][crate::Input].
pub struct State<Ctx>
where
    Ctx: Context,
{
    /// The context for the consensus state machine
    pub ctx: Ctx,

    /// The consensus parameters
    pub params: Params<Ctx>,

    /// Driver for the per-round consensus state machine
    pub driver: Driver<Ctx>,

    /// A queue of inputs that were received before the driver started.
    pub input_queue: MaxQueue<Ctx::Height, Input<Ctx>>,

    /// The proposals to decide on.
    pub full_proposal_keeper: FullProposalKeeper<Ctx>,

    /// Store Precommit votes to be sent along the decision to the host
    pub signed_precommits: BTreeMap<(Ctx::Height, Round), BTreeSet<SignedVote<Ctx>>>,

    /// Decision per height
    pub decision: BTreeMap<(Ctx::Height, Round), SignedProposal<Ctx>>,
}

impl<Ctx> State<Ctx>
where
    Ctx: Context,
{
    pub fn new(ctx: Ctx, params: Params<Ctx>) -> Self {
        let driver = Driver::new(
            ctx.clone(),
            params.initial_height,
            params.initial_validator_set.clone(),
            params.address.clone(),
            params.threshold_params,
        );

        Self {
            ctx,
            driver,
            params,
            input_queue: Default::default(),
            full_proposal_keeper: Default::default(),
            signed_precommits: Default::default(),
            decision: Default::default(),
        }
    }

    pub fn height(&self) -> Ctx::Height {
        self.driver.height()
    }

    pub fn round(&self) -> Round {
        self.driver.round()
    }

    pub fn address(&self) -> &Ctx::Address {
        self.driver.address()
    }

    pub fn validator_set(&self) -> &Ctx::ValidatorSet {
        self.driver.validator_set()
    }

    pub fn get_proposer(&self, height: Ctx::Height, round: Round) -> &Ctx::Address {
        self.ctx
            .select_proposer(self.validator_set(), height, round)
            .address()
    }

    pub fn store_signed_precommit(&mut self, precommit: SignedVote<Ctx>) {
        assert_eq!(precommit.vote_type(), VoteType::Precommit);

        let height = precommit.height();
        let round = precommit.round();

        self.signed_precommits
            .entry((height, round))
            .or_default()
            .insert(precommit);
    }

    pub fn store_decision(&mut self, height: Ctx::Height, round: Round, proposal: Ctx::Proposal) {
        if let Some(full_proposal) = self.full_proposal_keeper.full_proposal_at_round_and_value(
            &height,
            proposal.round(),
            &proposal.value().id(),
        ) {
            self.decision.insert(
                (self.driver.height(), round),
                full_proposal.proposal.clone(),
            );
        }
    }

    pub fn restore_precommits(
        &mut self,
        height: Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Vec<SignedVote<Ctx>> {
        // Get the commits for the height and round.
        let commits_for_height_and_round = self
            .signed_precommits
            .remove(&(height, round))
            .unwrap_or_default();

        // Keep the commits for the specified value.
        // For now, we ignore equivocating votes if present.
        commits_for_height_and_round
            .into_iter()
            .filter(|c| c.value() == &NilOrVal::Val(value.id()))
            .collect()
    }

    pub fn restore_votes(&mut self, height: Ctx::Height, round: Round) -> Vec<SignedVote<Ctx>> {
        // TODO optimization - get votes for all rounds higher than or equal to `round`
        if height != self.driver.height() {
            return vec![];
        }

        if let Some(per_round) = self.driver.votes().per_round(round) {
            per_round.received_votes().iter().cloned().collect()
        } else {
            vec![]
        }
    }

    pub fn full_proposal_at_round_and_value(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Option<&FullProposal<Ctx>> {
        self.full_proposal_keeper
            .full_proposal_at_round_and_value(height, round, &value.id())
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

        // Store the value at both round and valid_round
        self.full_proposal_keeper.store_value(new_value);
    }

    pub fn remove_full_proposals(&mut self, height: Ctx::Height) {
        debug!(%height, "Pruning full proposals");
        self.full_proposal_keeper.remove_full_proposals(height)
    }

    /// Queue an input for later processing, only keep inputs for the highest height seen so far.
    pub fn buffer_input(&mut self, height: Ctx::Height, input: Input<Ctx>) {
        self.input_queue.push(height, input);
    }

    pub fn print_state(&self) {
        if let Some(per_round) = self.driver.votes().per_round(self.driver.round()) {
            warn!(
                "Number of validators having voted: {} / {}",
                per_round.addresses_weights().get_inner().len(),
                self.driver.validator_set().count()
            );
            warn!(
                "Total voting power of validators: {}",
                self.driver.validator_set().total_voting_power()
            );
            warn!(
                "Voting power required: {}",
                self.driver.validator_set().total_voting_power() * 2 / 3
            );
            warn!(
                "Total voting power of validators having voted: {}",
                per_round.addresses_weights().sum()
            );
            warn!(
                "Total voting power of validators having prevoted nil: {}",
                per_round
                    .votes()
                    .get_weight(VoteType::Prevote, &NilOrVal::Nil)
            );
            warn!(
                "Total voting power of validators having precommited nil: {}",
                per_round
                    .votes()
                    .get_weight(VoteType::Precommit, &NilOrVal::Nil)
            );
            warn!(
                "Total weight of prevotes: {}",
                per_round.votes().weight_sum(VoteType::Prevote)
            );
            warn!(
                "Total weight of precommits: {}",
                per_round.votes().weight_sum(VoteType::Precommit)
            );
        }
    }

    /// Check if we are a validator node, i.e. we are present in the current validator set.
    pub fn is_validator(&self) -> bool {
        self.validator_set()
            .get_by_address(self.address())
            .is_some()
    }
}
