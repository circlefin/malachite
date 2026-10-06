//! Cross-checking re-derived consensus messages against the write-ahead log (WAL).
//!
//! Replaying the WAL re-derives every vote and proposal this node cast before the crash. For each
//! re-derivation, we use [`ReplayIndex`] to look up messages this node sent. If the recorded and
//! re-derived messages are equal, we reuse the recorded signature. If they are not equal, there is
//! a non-determinism bug in the consensus code, so we stop to avoid double-signing.

use core::fmt;
use std::collections::BTreeMap;

use derive_where::derive_where;
use tracing::error;

use malachitebft_core_types::{
    Context, Proposal, Round, SignedProposal, SignedVote, Vote, VoteType,
};

use crate::input::Input;

/// The kind of message a [`ReplayIndex`] record holds.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordKind {
    /// A prevote cast by this node.
    Prevote,
    /// A precommit cast by this node.
    Precommit,
    /// A proposal made by this node.
    Proposal,
}

impl RecordKind {
    /// The record kind a vote of this type is filed under.
    pub fn of_vote(vote_type: VoteType) -> Self {
        match vote_type {
            VoteType::Prevote => Self::Prevote,
            VoteType::Precommit => Self::Precommit,
        }
    }
}

impl fmt::Display for RecordKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prevote => write!(f, "prevote"),
            Self::Precommit => write!(f, "precommit"),
            Self::Proposal => write!(f, "proposal"),
        }
    }
}

/// Whether a re-derived vote disagrees with the one recorded on the WAL.
/// The vote extension is excluded from the comparison because it is non-deterministic.
pub fn votes_diverge<Ctx>(derived: &Ctx::Vote, recorded: &Ctx::Vote) -> bool
where
    Ctx: Context,
{
    let mut derived_bare = derived.clone();
    let mut recorded_bare = recorded.clone();
    derived_bare.take_extension();
    recorded_bare.take_extension();

    derived_bare != recorded_bare
}

/// Report that the log holds two conflicting messages of ours under one key.
fn report_double_sign(kind: RecordKind, height: impl fmt::Display, round: Round) {
    error!(
        %kind, %height, %round,
        "Write-ahead log holds two conflicting messages of ours for the same height, round and \
         kind: this node double-signed. Keeping the one recorded first"
    );
}

/// The messages this node singed, sent, and recorded in the write-ahead log for the height being replayed.
/// The struct is empty outside of replay.
#[derive_where(Clone, Debug, Default)]
pub struct ReplayIndex<Ctx>
where
    Ctx: Context,
{
    votes: BTreeMap<(VoteType, Ctx::Height, Round), SignedVote<Ctx>>,
    proposals: BTreeMap<(Ctx::Height, Round), SignedProposal<Ctx>>,
}

impl<Ctx> ReplayIndex<Ctx>
where
    Ctx: Context,
{
    /// Index the messages recorded in `entries` filtered by the given `address`.
    pub fn from_wal_entries<'a, I>(entries: I, address: &Ctx::Address) -> Self
    where
        I: IntoIterator<Item = &'a Input<Ctx>>,
        Ctx: 'a,
    {
        let mut index = Self::default();

        for entry in entries {
            match entry {
                Input::Vote(vote) if vote.validator_address() == address => index.insert_vote(vote),

                Input::Proposal(proposal) if proposal.validator_address() == address => {
                    index.insert_proposal(proposal)
                }

                _ => (),
            }
        }

        index
    }

    /// The vote recorded for the same type, height and round as `vote`, if any.
    pub fn recorded_vote(&self, vote: &Ctx::Vote) -> Option<&SignedVote<Ctx>> {
        self.votes
            .get(&(vote.vote_type(), vote.height(), vote.round()))
    }

    /// The proposal recorded for the same height and round as `proposal`, if any.
    pub fn recorded_proposal(&self, proposal: &Ctx::Proposal) -> Option<&SignedProposal<Ctx>> {
        self.proposals.get(&(proposal.height(), proposal.round()))
    }

    fn insert_vote(&mut self, vote: &SignedVote<Ctx>) {
        let key = (vote.vote_type(), vote.height(), vote.round());

        let take_incoming = match self.votes.get(&key) {
            None => true,

            Some(existing) if votes_diverge::<Ctx>(&vote.message, &existing.message) => {
                // Keep the vote we recorded first
                report_double_sign(RecordKind::of_vote(key.0), key.1, key.2);
                false
            }

            // Keep the vote that contains the extension.
            Some(existing) => existing.extension().is_none() && vote.extension().is_some(),
        };

        if take_incoming {
            self.votes.insert(key, vote.clone());
        }
    }

    fn insert_proposal(&mut self, proposal: &SignedProposal<Ctx>) {
        let key = (proposal.height(), proposal.round());

        match self.proposals.get(&key) {
            None => {
                self.proposals.insert(key, proposal.clone());
            }

            // Keep the proposal recorded first, as for votes.
            Some(existing) => {
                if proposal.message != existing.message {
                    report_double_sign(RecordKind::Proposal, key.0, key.1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use malachitebft_core_types::{NilOrVal, SignedMessage, SignedVote, Timeout};
    use malachitebft_test::{Address, Height, Signature, TestContext, ValueId, Vote};

    use super::*;

    const HEIGHT: u64 = 1;

    fn address(byte: u8) -> Address {
        Address::new([byte; 20])
    }

    fn prevote(value: NilOrVal<ValueId>, addr: Address) -> Vote {
        Vote::new_prevote(Height::new(HEIGHT), Round::new(0), value, addr)
    }

    fn signed(vote: Vote, signature: u8) -> SignedVote<TestContext> {
        SignedVote::new(vote, Signature::from_bytes([signature; 64]))
    }

    fn with_extension(mut vote: Vote, extension: &'static [u8]) -> Vote {
        vote.extension = Some(SignedMessage::new(
            Bytes::from_static(extension),
            Signature::test(),
        ));
        vote
    }

    #[test]
    fn identical_votes_do_not_diverge() {
        let vote = prevote(NilOrVal::Val(ValueId::new(1)), address(1));

        assert!(!votes_diverge::<TestContext>(&vote, &vote.clone()));
    }

    #[test]
    fn votes_differing_only_in_their_extension_are_not_divergent() {
        let vote = prevote(NilOrVal::Val(ValueId::new(1)), address(1));

        assert!(!votes_diverge::<TestContext>(
            &with_extension(vote.clone(), b"derived"),
            &with_extension(vote.clone(), b"recorded"),
        ));

        // Gaining or losing an extension counts the same: consensus does not act on it.
        assert!(!votes_diverge::<TestContext>(
            &vote,
            &with_extension(vote.clone(), b"recorded")
        ));
    }

    #[test]
    fn votes_differing_in_their_value_are_divergent() {
        let addr = address(1);

        assert!(votes_diverge::<TestContext>(
            &prevote(NilOrVal::Nil, addr),
            &prevote(NilOrVal::Val(ValueId::new(1)), addr),
        ));
    }

    #[test]
    fn only_our_own_messages_are_indexed() {
        let me = address(1);
        let peer = address(2);
        let our_vote = signed(prevote(NilOrVal::Nil, me), 0xaa);
        let their_vote = signed(prevote(NilOrVal::Val(ValueId::new(1)), peer), 0xbb);

        let index = ReplayIndex::<TestContext>::from_wal_entries(
            &[
                Input::Vote(their_vote),
                Input::Vote(our_vote.clone()),
                Input::TimeoutElapsed(Timeout::propose(Round::new(0))),
            ],
            &me,
        );

        assert_eq!(index.recorded_vote(&our_vote.message), Some(&our_vote));
    }

    #[test]
    fn a_key_recorded_twice_differing_only_in_extension_keeps_the_extended_copy() {
        let me = address(1);
        // A historical WAL may hold the same precommit twice: the extended copy we signed,
        // then an unextended reconstruction from a round certificate (`RoundSignature`
        // carries none). The extended copy is the one we actually sent.
        let cast = signed(
            with_extension(prevote(NilOrVal::Nil, me), b"from-the-app"),
            0xaa,
        );
        let reconstructed = signed(prevote(NilOrVal::Nil, me), 0xaa);

        for order in [
            [cast.clone(), reconstructed.clone()],
            [reconstructed.clone(), cast.clone()],
        ] {
            let index = ReplayIndex::<TestContext>::from_wal_entries(&order.map(Input::Vote), &me);

            assert_eq!(
                index.recorded_vote(&cast.message),
                Some(&cast),
                "the key must not be poisoned, and the extended copy must win"
            );
        }
    }

    #[test]
    fn a_key_recorded_twice_with_conflicting_messages_keeps_the_first() {
        let me = address(1);
        // Only reachable if we already double-signed before the crash. Keeping neither would
        // leave the key unrecorded and send replay to the signer for a third message under it.
        let first = signed(prevote(NilOrVal::Nil, me), 0xaa);
        let second = signed(prevote(NilOrVal::Val(ValueId::new(1)), me), 0xbb);

        let index = ReplayIndex::<TestContext>::from_wal_entries(
            &[Input::Vote(first.clone()), Input::Vote(second.clone())],
            &me,
        );

        // Both look up the same key, since they differ only in the value voted for.
        assert_eq!(index.recorded_vote(&first.message), Some(&first));
        assert_eq!(
            index.recorded_vote(&second.message),
            Some(&first),
            "a conflicting second record must not displace the first, nor clear the key"
        );
    }

    #[test]
    fn the_same_message_recorded_twice_stays_usable() {
        let me = address(1);
        let vote = signed(prevote(NilOrVal::Nil, me), 0xaa);

        let index = ReplayIndex::<TestContext>::from_wal_entries(
            &[Input::Vote(vote.clone()), Input::Vote(vote.clone())],
            &me,
        );

        assert_eq!(index.recorded_vote(&vote.message), Some(&vote));
    }

    #[test]
    fn a_prevote_and_a_precommit_for_the_same_round_do_not_share_a_key() {
        let me = address(1);
        let value = ValueId::new(1);
        let our_prevote = signed(prevote(NilOrVal::Val(value), me), 0xaa);
        let our_precommit = signed(
            Vote::new_precommit(Height::new(HEIGHT), Round::new(0), NilOrVal::Val(value), me),
            0xbb,
        );

        let index = ReplayIndex::<TestContext>::from_wal_entries(
            &[
                Input::Vote(our_prevote.clone()),
                Input::Vote(our_precommit.clone()),
            ],
            &me,
        );

        assert_eq!(
            index.recorded_vote(&our_prevote.message),
            Some(&our_prevote)
        );
        assert_eq!(
            index.recorded_vote(&our_precommit.message),
            Some(&our_precommit)
        );
    }
}
