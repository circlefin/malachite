use malachite_common::VoteType;
use malachite_vote::round_votes::RoundVotes;
use malachite_vote::Threshold;

use malachite_test::{Address, ValueId};

const ADDRESS1: Address = Address::new([41; 20]);
const ADDRESS2: Address = Address::new([42; 20]);
const ADDRESS3: Address = Address::new([43; 20]);
const ADDRESS4: Address = Address::new([44; 20]);
const ADDRESS5: Address = Address::new([45; 20]);
const ADDRESS6: Address = Address::new([46; 20]);

#[test]
fn add_votes_nil() {
    let total = 3;

    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new(total, Default::default());

    // add a vote for nil. nothing changes.
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS1, None, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, nothing changes.
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS2, None, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, get Nil
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS3, None, 1);
    assert_eq!(thresh, Threshold::Nil);
}

#[test]
fn add_votes_single_value() {
    let v = ValueId::new(1);
    let val = Some(v);
    let total = 4;
    let weight = 1;

    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new(total, Default::default());

    // add a vote. nothing changes.
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS1, val, weight);
    assert_eq!(thresh, Threshold::Unreached);

    // add it again, nothing changes.
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS2, val, weight);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for nil, get Thresh::Any
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS3, None, weight);
    assert_eq!(thresh, Threshold::Any);

    // add vote for value, get Thresh::Value
    let thresh = round_votes.add_vote(VoteType::Prevote, ADDRESS4, val, weight);
    assert_eq!(thresh, Threshold::Value(v));
}

#[test]
fn add_votes_multi_values() {
    let v1 = ValueId::new(1);
    let v2 = ValueId::new(2);
    let val1 = Some(v1);
    let val2 = Some(v2);
    let total = 15;

    let mut round_votes: RoundVotes<_, ValueId> = RoundVotes::new(total, Default::default());

    // add a vote for v1. nothing changes.
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS1, val1, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v2. nothing changes.
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS2, val2, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for nil. nothing changes.
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS3, None, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v1. nothing changes
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS4, val1, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a vote for v2. nothing changes
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS5, val2, 1);
    assert_eq!(thresh, Threshold::Unreached);

    // add a big vote for v2. get Value(v2)
    let thresh = round_votes.add_vote(VoteType::Precommit, ADDRESS6, val2, 10);
    assert_eq!(thresh, Threshold::Value(v2));
}
