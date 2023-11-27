use malachite_test::{Address, Height, Proposal, TestContext, Value};

use malachite_common::{Round, Timeout, TimeoutStep};
use malachite_round::events::Event;
use malachite_round::message::Message;
use malachite_round::state::{State, Step};
use malachite_round::state_machine::{apply_event, Info};

const ADDRESS: Address = Address::new([42; 20]);
const OTHER_ADDRESS: Address = Address::new([21; 20]);

#[test]
fn test_propose() {
    let value = Value::new(42);
    let height = Height::new(10);
    let round = Round::new(0);

    let mut state: State<TestContext> = State {
        height,
        round,
        ..Default::default()
    };

    // We are the proposer
    let data = Info::new(round, &ADDRESS, &ADDRESS);

    let transition = apply_event(state.clone(), &data, Event::NewRound);

    state.step = Step::Propose;
    assert_eq!(transition.next_state, state);
    assert_eq!(
        transition.message.unwrap(),
        Message::get_value_and_schedule_timeout(round, TimeoutStep::Propose)
    );

    let transition = apply_event(transition.next_state, &data, Event::ProposeValue(value));

    state.step = Step::Propose;
    assert_eq!(transition.next_state, state);
    assert_eq!(
        transition.message.unwrap(),
        Message::proposal(Height::new(10), Round::new(0), Value::new(42), Round::Nil)
    );
}

#[test]
fn test_prevote() {
    let value = Value::new(42);
    let height = Height::new(1);
    let round = Round::new(1);

    let state: State<TestContext> = State {
        height,
        round,
        ..Default::default()
    };

    // We are not the proposer
    let data = Info::new(Round::new(1), &ADDRESS, &OTHER_ADDRESS);

    let transition = apply_event(state, &data, Event::NewRound);

    assert_eq!(transition.next_state.step, Step::Propose);
    assert_eq!(
        transition.message.unwrap(),
        Message::ScheduleTimeout(Timeout {
            round: Round::new(1),
            step: TimeoutStep::Propose
        })
    );

    let state = transition.next_state;

    let transition = apply_event(
        state,
        &data,
        Event::Proposal(Proposal::new(
            Height::new(1),
            Round::new(1),
            value,
            Round::Nil,
        )),
    );

    assert_eq!(transition.next_state.step, Step::Prevote);
    assert_eq!(
        transition.message.unwrap(),
        Message::prevote(Height::new(1), Round::new(1), Some(value.id()), ADDRESS)
    );
}
