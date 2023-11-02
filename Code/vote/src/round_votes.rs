use std::collections::{BTreeMap, BTreeSet};

use malachite_common::{Context, Round, ValueId, Vote, VoteType};

use crate::count::VoteCount;
use crate::{Threshold, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
pub struct RoundVotes<Ctx>
where
    Ctx: Context,
{
    pub height: Ctx::Height,
    pub round: Round,
    pub total_weight: Weight,

    pub prevotes: VoteCount<Ctx::Address, ValueId<Ctx>>,
    pub precommits: VoteCount<Ctx::Address, ValueId<Ctx>>,

    pub emitted_thresholds: BTreeSet<Threshold<ValueId<Ctx>>>,
    pub votes_addresses_weights: BTreeMap<Ctx::Address, Weight>,
}

impl<Ctx> RoundVotes<Ctx>
where
    Ctx: Context,
{
    pub fn new(height: Ctx::Height, round: Round, total_weight: Weight) -> Self {
        RoundVotes {
            height,
            round,
            total_weight,
            prevotes: VoteCount::new(total_weight),
            precommits: VoteCount::new(total_weight),
            emitted_thresholds: BTreeSet::new(),
            votes_addresses_weights: BTreeMap::new(),
        }
    }

    pub fn add_vote(&mut self, vote: Ctx::Vote, weight: Weight) -> Threshold<ValueId<Ctx>> {
        let address = vote.validator_address().clone();

        let threshold = match vote.vote_type() {
            VoteType::Prevote => self
                .prevotes
                .add(address.clone(), vote.take_value(), weight),
            VoteType::Precommit => self
                .precommits
                .add(address.clone(), vote.take_value(), weight),
        };

        // Store the weight of that validator, if that's the first vote we see from it
        self.votes_addresses_weights
            .entry(address)
            .or_insert(weight);

        let sum_skip = self.votes_addresses_weights.values().sum::<Weight>();

        let final_threshold = if !self.already_emitted(&threshold) {
            threshold
        } else if is_skip(sum_skip, self.total_weight) && !self.already_emitted(&Threshold::Skip) {
            Threshold::Skip
        } else {
            Threshold::Unreached
        };

        self.emit(final_threshold)
    }

    fn emit(&mut self, threshold: Threshold<ValueId<Ctx>>) -> Threshold<ValueId<Ctx>> {
        self.emitted_thresholds.insert(threshold.clone());
        threshold
    }

    fn already_emitted(&self, threshold: &Threshold<ValueId<Ctx>>) -> bool {
        self.emitted_thresholds.contains(threshold)
    }
}

fn is_skip(weight: Weight, total: Weight) -> bool {
    3 * weight > total
}
