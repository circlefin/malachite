//! Tally votes of the same type (eg. prevote or precommit)

#![forbid(unsafe_code)]
#![deny(unused_crate_dependencies, trivial_casts, trivial_numeric_casts)]
#![warn(
    // missing_docs,
    rustdoc::broken_intra_doc_links,
    rustdoc::private_intra_doc_links,
    variant_size_differences
)]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::panic))]

extern crate alloc;

pub mod count;
pub mod keeper;

use malachite_common::{Consensus, Round, ValueId, Vote, VoteType};

use crate::count::{Threshold, VoteCount, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
pub struct RoundVotes<C>
where
    C: Consensus,
{
    pub height: C::Height,
    pub round: Round,

    pub prevotes: VoteCount<C>,
    pub precommits: VoteCount<C>,
}

impl<C> RoundVotes<C>
where
    C: Consensus,
{
    pub fn new(height: C::Height, round: Round, total: Weight) -> Self {
        RoundVotes {
            height,
            round,
            prevotes: VoteCount::new(total),
            precommits: VoteCount::new(total),
        }
    }

    pub fn add_vote(&mut self, vote: C::Vote, weight: Weight) -> Threshold<ValueId<C>> {
        match vote.vote_type() {
            VoteType::Prevote => self.prevotes.add_vote(vote, weight),
            VoteType::Precommit => self.precommits.add_vote(vote, weight),
        }
    }
}
