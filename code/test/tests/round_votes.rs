use malachite_common::{NilOrVal, VoteType};
use malachite_vote::round_votes::RoundVotes;

use malachite_test::{Address, ValueId};

const ADDRESS1: Address = Address::new([41; 20]);
const ADDRESS2: Address = Address::new([42; 20]);
const ADDRESS3: Address = Address::new([43; 20]);
const ADDRESS4: Address = Address::new([44; 20]);
const ADDRESS5: Address = Address::new([45; 20]);
const ADDRESS6: Address = Address::new([46; 20]);

#[test]
fn add_votes_nil() {
    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new();

    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS1, NilOrVal::Nil, 1);
    assert_eq!(w, 1);

    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS2, NilOrVal::Nil, 1);
    assert_eq!(w, 2);

    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS3, NilOrVal::Nil, 1);
    assert_eq!(w, 3);
}

#[test]
fn add_votes_single_value() {
    let v = ValueId::new(1);
    let val = NilOrVal::Val(v);
    let weight = 1;

    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new();

    // add a vote. nothing changes.
    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS1, val, weight);
    assert_eq!(w, 1);

    // add it again, nothing changes.
    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS2, val, weight);
    assert_eq!(w, 2);

    // add a vote for nil, get w::Any
    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS3, NilOrVal::Nil, weight);
    assert_eq!(w, 1);

    // add vote for value, get w::Value
    let w = round_votes.add_vote(VoteType::Prevote, ADDRESS4, val, weight);
    assert_eq!(w, 3);
}

#[test]
fn add_votes_multi_values() {
    let v1 = ValueId::new(1);
    let v2 = ValueId::new(2);
    let val1 = NilOrVal::Val(v1);
    let val2 = NilOrVal::Val(v2);

    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new();

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS1, val1, 1);
    assert_eq!(w, 1);

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS2, val2, 1);
    assert_eq!(w, 1);

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS3, NilOrVal::Nil, 1);
    assert_eq!(w, 1);

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS4, val1, 1);
    assert_eq!(w, 2);

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS5, val2, 1);
    assert_eq!(w, 2);

    let w = round_votes.add_vote(VoteType::Precommit, ADDRESS6, val2, 10);
    assert_eq!(w, 12);
}
