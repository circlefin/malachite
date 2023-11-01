use malachite_common::Round;
use malachite_vote::count::Threshold;
use malachite_vote::RoundVotes;

use malachite_test::{Address, Height, TestContext, ValueId, Vote};

const ADDRESS: Address = Address::new([42; 20]);

#[test]
fn add_votes_nil() {
    let total = 3;

    let mut round_votes: RoundVotes<TestContext> =
        RoundVotes::new(Height::new(1), Round::new(0), total);

    // add a vote for nil. nothing changes.
    let vote = Vote::new_prevote(Round::new(0), None, ADDRESS);
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
    let v = ValueId::new(1);
    let val = Some(v);
    let total = 4;
    let weight = 1;

    let mut round_votes: RoundVotes<TestContext> =
        RoundVotes::new(Height::new(1), Round::new(0), total);

    // add a vote. nothing changes.
    let vote = Vote::new_prevote(Round::new(0), val, ADDRESS);
    let thresh = round_votes.add_vote(vote.clone(), weight);
    assert_eq!(thresh, Threshold::Init);

    // add it again, nothing changes.
    let thresh = round_votes.add_vote(vote.clone(), weight);
    assert_eq!(thresh, Threshold::Init);

    // add a vote for nil, get Thresh::Any
    let vote_nil = Vote::new_prevote(Round::new(0), None, ADDRESS);
    let thresh = round_votes.add_vote(vote_nil, weight);
    assert_eq!(thresh, Threshold::Any);

    // add vote for value, get Thresh::Value
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
    let vote1 = Vote::new_precommit(Round::new(0), val1, ADDRESS);
    let thresh = round_votes.add_vote(vote1.clone(), 1);
    assert_eq!(thresh, Threshold::Init);

    // add a vote for v2. nothing changes.
    let vote2 = Vote::new_precommit(Round::new(0), val2, ADDRESS);
    let thresh = round_votes.add_vote(vote2.clone(), 1);
    assert_eq!(thresh, Threshold::Init);

    // add a vote for nil. nothing changes.
    let vote_nil = Vote::new_precommit(Round::new(0), None, ADDRESS);
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
    assert_eq!(thresh, Threshold::Value(v2));
}
