use malachite_common::Round;
use malachite_vote::keeper::{Message, VoteKeeper};

use malachite_test::{Address, Height, TestContext, ValueId, Vote};

const ADDRESS: Address = Address::new([42; 20]);

#[test]
fn prevote_apply_nil() {
    let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

    let vote = Vote::new_prevote(Round::new(0), None, ADDRESS);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote, 1);
    assert_eq!(msg, Some(Message::PolkaNil));
}

#[test]
fn precommit_apply_nil() {
    let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

    let vote = Vote::new_precommit(Round::new(0), None, ADDRESS);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote, 1);
    assert_eq!(msg, Some(Message::PrecommitAny));
}

#[test]
fn prevote_apply_single_value() {
    let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

    let v = ValueId::new(1);
    let val = Some(v);
    let vote = Vote::new_prevote(Round::new(0), val, ADDRESS);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let vote_nil = Vote::new_prevote(Round::new(0), None, ADDRESS);
    let msg = keeper.apply_vote(vote_nil, 1);
    assert_eq!(msg, Some(Message::PolkaAny));

    let msg = keeper.apply_vote(vote, 1);
    assert_eq!(msg, Some(Message::PolkaValue(v)));
}

#[test]
fn precommit_apply_single_value() {
    let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

    let v = ValueId::new(1);
    let val = Some(v);
    let vote = Vote::new_precommit(Round::new(0), val, ADDRESS);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let msg = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(msg, None);

    let vote_nil = Vote::new_precommit(Round::new(0), None, ADDRESS);
    let msg = keeper.apply_vote(vote_nil, 1);
    assert_eq!(msg, Some(Message::PrecommitAny));

    let msg = keeper.apply_vote(vote, 1);
    assert_eq!(msg, Some(Message::PrecommitValue(v)));
}
