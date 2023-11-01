use malachite_common::Round;
use malachite_vote::count::Threshold;
use malachite_vote::RoundVotes;

use malachite_test::{Address, Height, TestContext, ValueId, Vote};

const ADDRESS1: Address = Address::new([41; 20]);
const ADDRESS2: Address = Address::new([42; 20]);
const ADDRESS3: Address = Address::new([43; 20]);
const ADDRESS4: Address = Address::new([44; 20]);
const ADDRESS5: Address = Address::new([45; 20]);
const ADDRESS6: Address = Address::new([46; 20]);

#[test]
fn add_votes_nil() {
    let total = 3;

    let mut round_votes: RoundVotes<TestContext> =
        RoundVotes::new(Height::new(1), Round::new(0), total);

    // add a vote for nil. nothing changes.
    let vote = Vote::new_prevote(Round::new(0), None, ADDRESS1);
    let thresh = round_votes.add_vote(vote.clone(), 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, nothing changes.
    let vote = Vote::new_prevote(Round::new(0), None, ADDRESS2);
    let thresh = round_votes.add_vote(vote, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, get Nil
    let vote = Vote::new_prevote(Round::new(0), None, ADDRESS3);
    let thresh = round_votes.add_vote(vote, 1);
    assert_eq!(thresh, Threshold::Nil);
}

#[test]
fn add_votes_single_value() {
    let v = ValueId::new(1);
    let val = Some(v);
    let total = 4;
    let weight = 1;

    let mut round_votes: RoundVotes<TestContext> =
        RoundVotes::new(Height::new(1), Round::new(0), total);

    // add a vote. nothing changes.
    let vote = Vote::new_prevote(Round::new(0), val, ADDRESS1);
    let thresh = round_votes.add_vote(vote.clone(), weight);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, nothing changes.
    let vote = Vote::new_prevote(Round::new(0), val, ADDRESS2);
    let thresh = round_votes.add_vote(vote.clone(), weight);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for nil, get Thresh::Any
    let vote_nil = Vote::new_prevote(Round::new(0), None, ADDRESS3);
    let thresh = round_votes.add_vote(vote_nil, weight);
    assert_eq!(thresh, Threshold::Any);

    // add vote for value, get Thresh::Value
    let vote = Vote::new_prevote(Round::new(0), val, ADDRESS4);
    let thresh = round_votes.add_vote(vote, weight);
    assert_eq!(thresh, Threshold::Value(v));
}

#[test]
fn add_votes_multi_values() {
    let v1 = ValueId::new(1);
    let v2 = ValueId::new(2);
    let val1 = Some(v1);
    let val2 = Some(v2);
    let total = 15;

    let mut round_votes: RoundVotes<TestContext> =
        RoundVotes::new(Height::new(1), Round::new(0), total);

    // add a vote for v1. nothing changes.
    let vote1 = Vote::new_precommit(Round::new(0), val1, ADDRESS1);
    let thresh = round_votes.add_vote(vote1, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v2. nothing changes.
    let vote2 = Vote::new_precommit(Round::new(0), val2, ADDRESS2);
    let thresh = round_votes.add_vote(vote2, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for nil. nothing changes.
    let vote_nil = Vote::new_precommit(Round::new(0), None, ADDRESS3);
    let thresh = round_votes.add_vote(vote_nil, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v1. nothing changes
    let vote1 = Vote::new_precommit(Round::new(0), val1, ADDRESS4);
    let thresh = round_votes.add_vote(vote1, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v2. nothing changes
    let vote2 = Vote::new_precommit(Round::new(0), val2, ADDRESS5);
    let thresh = round_votes.add_vote(vote2, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a big vote for v2. get Value(v2)
    let vote2 = Vote::new_precommit(Round::new(0), val2, ADDRESS6);
    let thresh = round_votes.add_vote(vote2, 10);
    assert_eq!(thresh, Threshold::Value(v2));
}
