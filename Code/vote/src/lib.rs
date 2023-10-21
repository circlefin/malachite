//! Tally votes of the same type (eg. prevote or precommit)

extern crate alloc;

pub mod count;
pub mod keeper;

use malachite_common::{Height, Round, Vote, VoteType};

use crate::count::{Threshold, VoteCount, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
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
