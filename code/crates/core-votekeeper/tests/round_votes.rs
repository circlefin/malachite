use malachite_core_types::{NilOrVal, Round};
use malachite_core_votekeeper::round_votes::RoundVotes;

use malachite_test::{Address, Height, TestContext, ValueId, Vote};

const ADDRESS1: Address = Address::new([41; 20]);
const ADDRESS2: Address = Address::new([42; 20]);
const ADDRESS3: Address = Address::new([43; 20]);
const ADDRESS4: Address = Address::new([44; 20]);
const ADDRESS5: Address = Address::new([45; 20]);
const ADDRESS6: Address = Address::new([46; 20]);

#[test]
fn add_votes_nil() {
    let h = Height::new(1);
    let r = Round::new(0);

    let mut round_votes = RoundVotes::<TestContext>::new();

    let vote1 = Vote::new_prevote(h, r, NilOrVal::Nil, ADDRESS1);
    let weight1 = round_votes.add_vote(&vote1, 1);
    assert_eq!(weight1, 1);

    let vote2 = Vote::new_prevote(h, r, NilOrVal::Nil, ADDRESS2);
    let weight2 = round_votes.add_vote(&vote2, 1);
    assert_eq!(weight2, 2);

    let vote3 = Vote::new_prevote(h, r, NilOrVal::Nil, ADDRESS3);
    let weight3 = round_votes.add_vote(&vote3, 1);
    assert_eq!(weight3, 3);
}

#[test]
fn add_votes_single_value() {
    let h = Height::new(1);
    let r = Round::new(0);
    let v = ValueId::new(1);
    let val = NilOrVal::Val(v);
    let weight = 1;

    let mut round_votes = RoundVotes::<TestContext>::new();

    // add a vote, nothing changes.
    let vote1 = Vote::new_prevote(h, r, val, ADDRESS1);
    let weight1 = round_votes.add_vote(&vote1, weight);
    assert_eq!(weight1, 1);

    // add it again, nothing changes.
    let vote2 = Vote::new_prevote(h, r, val, ADDRESS2);
    let weight3 = round_votes.add_vote(&vote2, weight);
    assert_eq!(weight3, 2);

    // add a vote for nil, get w::Any
    let vote3 = Vote::new_prevote(h, r, NilOrVal::Nil, ADDRESS3);
    let weight4 = round_votes.add_vote(&vote3, weight);
    assert_eq!(weight4, 1);

    // add vote for value, get w::Value
    let vote5 = Vote::new_prevote(h, r, val, ADDRESS4);
    let weight5 = round_votes.add_vote(&vote5, weight);
    assert_eq!(weight5, 3);
}

#[test]
fn add_votes_multi_values() {
    let h = Height::new(1);
    let r = Round::new(0);

    let v1 = ValueId::new(1);
    let v2 = ValueId::new(2);
    let val1 = NilOrVal::Val(v1);
    let val2 = NilOrVal::Val(v2);

    let mut round_votes = RoundVotes::<TestContext>::new();

    let vote1 = Vote::new_precommit(h, r, val1, ADDRESS1);
    let weight1 = round_votes.add_vote(&vote1, 1);
    assert_eq!(weight1, 1);

    let vote2 = Vote::new_precommit(h, r, val2, ADDRESS2);
    let weight2 = round_votes.add_vote(&vote2, 1);
    assert_eq!(weight2, 1);

    let vote3 = Vote::new_precommit(h, r, NilOrVal::Nil, ADDRESS3);
    let weight3 = round_votes.add_vote(&vote3, 1);
    assert_eq!(weight3, 1);

    let vote4 = Vote::new_precommit(h, r, val1, ADDRESS4);
    let weight4 = round_votes.add_vote(&vote4, 1);
    assert_eq!(weight4, 2);

    let vote5 = Vote::new_precommit(h, r, val2, ADDRESS5);
    let weight5 = round_votes.add_vote(&vote5, 1);
    assert_eq!(weight5, 2);

    let vote6 = Vote::new_precommit(h, r, val2, ADDRESS6);
    let weight6 = round_votes.add_vote(&vote6, 10);
    assert_eq!(weight6, 12);
}
