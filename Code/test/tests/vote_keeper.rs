use malachite_common::Round;
use malachite_round::events::Event;
use malachite_vote::keeper::VoteKeeper;

use malachite_test::{Address, Height, TestConsensus, ValueId, Vote};

#[test]
fn prevote_apply_nil() {
    let mut keeper: VoteKeeper<TestConsensus> = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

    let vote = Vote::new_prevote(Round::new(0), None, Address::new(1));

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote, 1);
    assert_eq!(event, Some(Event::PolkaNil));
}

#[test]
fn precommit_apply_nil() {
    let mut keeper: VoteKeeper<TestConsensus> = VoteKeeper::new(Height::new(1), Round::INITIAL, 3);

    let vote = Vote::new_precommit(Round::new(0), None, Address::new(1));

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote, 1);
    assert_eq!(event, None);
}

#[test]
fn prevote_apply_single_value() {
    let mut keeper: VoteKeeper<TestConsensus> = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

    let v = ValueId::new(1);
    let val = Some(v);
    let vote = Vote::new_prevote(Round::new(0), val, Address::new(1));

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let vote_nil = Vote::new_prevote(Round::new(0), None, Address::new(2));
    let event = keeper.apply_vote(vote_nil, 1);
    assert_eq!(event, Some(Event::PolkaAny));

    let event = keeper.apply_vote(vote, 1);
    assert_eq!(event, Some(Event::PolkaValue(v)));
}

#[test]
fn precommit_apply_single_value() {
    let mut keeper: VoteKeeper<TestConsensus> = VoteKeeper::new(Height::new(1), Round::INITIAL, 4);

    let v = ValueId::new(1);
    let val = Some(v);
    let vote = Vote::new_precommit(Round::new(0), val, Address::new(1));

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let event = keeper.apply_vote(vote.clone(), 1);
    assert_eq!(event, None);

    let vote_nil = Vote::new_precommit(Round::new(0), None, Address::new(2));
    let event = keeper.apply_vote(vote_nil, 1);
    assert_eq!(event, Some(Event::PrecommitAny));

    let event = keeper.apply_vote(vote, 1);
    assert_eq!(event, Some(Event::PrecommitValue(v)));
}
