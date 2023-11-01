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

use malachite_common::{Context, Round, ValueId, Vote, VoteType};

use crate::count::{Threshold, VoteCount, Weight};

/// Tracks all the votes for a single round
#[derive(Clone, Debug)]
pub struct RoundVotes<Ctx>
where
    Ctx: Context,
{
    pub height: Ctx::Height,
    pub round: Round,

    pub prevotes: VoteCount<Ctx::Address, ValueId<Ctx>>,
    pub precommits: VoteCount<Ctx::Address, ValueId<Ctx>>,
}

impl<Ctx> RoundVotes<Ctx>
where
    Ctx: Context,
{
    pub fn new(height: Ctx::Height, round: Round, total: Weight) -> Self {
        RoundVotes {
            height,
            round,
            prevotes: VoteCount::new(total),
            precommits: VoteCount::new(total),
        }
    }

    pub fn add_vote(&mut self, vote: Ctx::Vote, weight: Weight) -> Threshold<ValueId<Ctx>> {
        match vote.vote_type() {
            VoteType::Prevote => {
                self.prevotes
                    .add(vote.validator_address().clone(), vote.take_value(), weight)
            }
            VoteType::Precommit => {
                self.precommits
                    .add(vote.validator_address().clone(), vote.take_value(), weight)
            }
        }
    }
}
