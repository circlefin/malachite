//! Tally votes of the same type (eg. prevote or precommit)

extern crate alloc;

use std::sync::Arc;

use alloc::collections::BTreeMap;

use malachite_common::{Height, Round, Value, Vote, VoteType};

pub type Weight = u64;

/// A value and the weight of votes for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValuesWeights {
    value_weights: BTreeMap<Arc<Value>, Weight>,
}

impl ValuesWeights {
    pub fn new() -> ValuesWeights {
        ValuesWeights {
            value_weights: BTreeMap::new(),
        }
    }

    pub fn add_weight(&mut self, value: Arc<Value>, weight: Weight) -> Weight {
        let entry = self.value_weights.entry(value).or_insert(0);
        *entry += weight;
        *entry
    }

    // pub fn weight_for(&self, value: &Value) -> Weight {
    //     self.value_weights.get(value).copied().unwrap_or(0)
    // }

    pub fn highest_weighted_value(&self) -> Option<(&Value, Weight)> {
        self.value_weights
            .iter()
            .max_by_key(|(_, weight)| *weight)
            .map(|(value, weight)| (value.as_ref(), *weight))
    }
}

impl Default for ValuesWeights {
    fn default() -> Self {
        Self::new()
    }
}

/// VoteCount tallys votes of the same type.
/// Votes are for nil or for some value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VoteCount {
    // Weight of votes for nil
    pub nil: Weight,
    /// Weight of votes for the values
    pub values_weights: ValuesWeights,
    /// Total weight
    pub total: Weight,
}

impl VoteCount {
    pub fn new(total: Weight) -> VoteCount {
        VoteCount {
            nil: 0,
            total,
            values_weights: ValuesWeights::new(),
        }
    }

    /// Add vote to internal counters and return the highest threshold.
    pub fn add_vote(&mut self, vote: Vote, weight: Weight) -> Threshold {
        if let Some(value) = vote.value {
            let value = Arc::new(value);
            let new_weight = self.values_weights.add_weight(value.clone(), weight);

            // Check if we have a quorum for this value.
            if is_quorum(new_weight, self.total) {
                return Threshold::Value(value);
            }
        } else {
            self.nil += weight;

            // Check if we have a quorum for nil.
            if is_quorum(self.nil, self.total) {
                return Threshold::Nil;
            }
        }

        // Check if we have a quorum for any value, using the highest weighted value, if any.
        if let Some((_max_value, max_weight)) = self.values_weights.highest_weighted_value() {
            if is_quorum(max_weight + self.nil, self.total) {
                return Threshold::Any;
            }
        }

        // No quorum
        Threshold::Init
    }
}

//-------------------------------------------------------------------------
// Round votes
//-------------------------------------------------------------------------

// Thresh represents the different quorum thresholds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Threshold {
    /// No quorum
    Init, // no quorum
    /// Qorum of votes but not for the same value
    Any,
    /// Quorum for nil
    Nil,
    /// Quorum for a value
    Value(Arc<Value>),
}

/// Returns whether or note `value > (2/3)*total`.
pub fn is_quorum(value: Weight, total: Weight) -> bool {
    3 * value > 2 * total
}

/// Tracks all the votes for a single round
pub struct RoundVotes {
    pub height: Height,
    pub round: Round,

    pub prevotes: VoteCount,
    pub precommits: VoteCount,
}

impl RoundVotes {
    pub fn new(height: Height, round: Round, total: Weight) -> RoundVotes {
        RoundVotes {
            height,
            round,
            prevotes: VoteCount::new(total),
            precommits: VoteCount::new(total),
        }
    }

    pub fn add_vote(&mut self, vote: Vote, weight: Weight) -> Threshold {
        match vote.typ {
            VoteType::Prevote => self.prevotes.add_vote(vote, weight),
            VoteType::Precommit => self.precommits.add_vote(vote, weight),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_votes_nil() {
        let total = 3;

        let mut round_votes = RoundVotes::new(Height::new(1), Round::new(0), total);

        // add a vote for nil. nothing changes.
        let vote = Vote::new_prevote(Round::new(0), None);
        let thresh = round_votes.add_vote(vote.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add it again, nothing changes.
        let thresh = round_votes.add_vote(vote.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add it again, get Nil
        let thresh = round_votes.add_vote(vote.clone(), 1);
        assert_eq!(thresh, Threshold::Nil);
    }

    #[test]
    fn add_votes_single_value() {
        let v = Value::new(1);
        let val = Some(v.clone());
        let total = 4;
        let weight = 1;

        let mut round_votes = RoundVotes::new(Height::new(1), Round::new(0), total);

        // add a vote. nothing changes.
        let vote = Vote::new_prevote(Round::new(0), val);
        let thresh = round_votes.add_vote(vote.clone(), weight);
        assert_eq!(thresh, Threshold::Init);

        // add it again, nothing changes.
        let thresh = round_votes.add_vote(vote.clone(), weight);
        assert_eq!(thresh, Threshold::Init);

        // add a vote for nil, get Thresh::Any
        let vote_nil = Vote::new_prevote(Round::new(0), None);
        let thresh = round_votes.add_vote(vote_nil, weight);
        assert_eq!(thresh, Threshold::Any);

        // add vote for value, get Thresh::Value
        let thresh = round_votes.add_vote(vote, weight);
        assert_eq!(thresh, Threshold::Value(Arc::new(v)));
    }

    #[test]
    fn add_votes_multi_values() {
        let v1 = Value::new(1);
        let v2 = Value::new(2);
        let val1 = Some(v1.clone());
        let val2 = Some(v2.clone());
        let total = 15;

        let mut round_votes = RoundVotes::new(Height::new(1), Round::new(0), total);

        // add a vote for v1. nothing changes.
        let vote1 = Vote::new_precommit(Round::new(0), val1);
        let thresh = round_votes.add_vote(vote1.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add a vote for v2. nothing changes.
        let vote2 = Vote::new_precommit(Round::new(0), val2);
        let thresh = round_votes.add_vote(vote2.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add a vote for nil. nothing changes.
        let vote_nil = Vote::new_precommit(Round::new(0), None);
        let thresh = round_votes.add_vote(vote_nil.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add a vote for v1. nothing changes
        let thresh = round_votes.add_vote(vote1.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add a vote for v2. nothing changes
        let thresh = round_votes.add_vote(vote2.clone(), 1);
        assert_eq!(thresh, Threshold::Init);

        // add a big vote for v2. get Value(v2)
        let thresh = round_votes.add_vote(vote2.clone(), 10);
        assert_eq!(thresh, Threshold::Value(Arc::new(v2)));
    }
}
