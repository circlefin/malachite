use std::collections::HashMap;
use std::ops::Deref;
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_common::{Context, Round, Value};
use malachite_itf::votekeeper::{State as ItfState, Value as ItfValue};
use malachite_itf::TraceRunner;
use malachite_test::{Address, Height, PrivateKey, TestContext, ValueId, Vote};
use malachite_vote::keeper::{Message, VoteKeeper};
use malachite_vote::{ThresholdParams, Weight};

use itf::Itf;
use rstest::{fixture, rstest};

const ADDRESSES: [&str; 3] = ["alice", "bob", "john"];
const NIL_VALUE: &str = "nil";

// TODO: move to itf-rs repo
fn from_itf<T, U>(bigint: &Itf<T>) -> Option<U>
where
    U: TryFrom<T>,
    T: Clone,
{
    bigint.deref().clone().try_into().ok()
}

fn value_from_model(value: &ItfValue) -> Option<ValueId> {
    match value.as_str() {
        NIL_VALUE => None,
        "proposal" => Some(0.into()),
        "val1" => Some(1.into()),
        "val2" => Some(2.into()),
        "val3" => Some(3.into()),
        _ => unimplemented!("unknown value {value:?}"),
    }
}

struct VoteKeeperRunner {
    address_map: HashMap<String, Address>,
}

impl TraceRunner for VoteKeeperRunner {
    type State = VoteKeeper<TestContext>;
    type Result = Option<Message<<<TestContext as Context>::Value as Value>::Id>>;

    type ExpectedState = ItfState;
    type Error = ();

    fn init(&mut self, expected_state: &Self::ExpectedState) -> Result<Self::State, Self::Error> {
        // Obtain the initial total_weight from the first state in the model.
        let total_weight: Weight = from_itf(&expected_state.bookkeeper.total_weight).unwrap();
        Ok(VoteKeeper::new(total_weight, ThresholdParams::default()))
    }

    fn step(
        &mut self,
        state: &mut Self::State,
        expected_state: &Self::ExpectedState,
    ) -> Result<Self::Result, Self::Error> {
        // Build step to execute.
        let (input_vote, weight) = expected_state.weighted_vote.deref();
        let round = Round::new(from_itf(&input_vote.round).unwrap());
        let height = Height::new(from_itf(&input_vote.height).unwrap());
        let value = value_from_model(&input_vote.value);
        let address = self.address_map.get(input_vote.address.as_str()).unwrap();
        let vote = match input_vote.typ.as_str() {
            "Prevote" => Vote::new_prevote(height, round, value, *address),
            "Precommit" => Vote::new_precommit(height, round, value, *address),
            _ => unreachable!(),
        };
        let weight: Weight = from_itf(weight).unwrap();
        println!(
            "🔵 step: vote={:?}, round={:?}, value={:?}, address={:?}, weight={:?}",
            input_vote.typ, round, value, input_vote.address, weight
        );

        let current_round = Round::new(from_itf(&expected_state.bookkeeper.current_round).unwrap());

        // Execute step.
        Ok(state.apply_vote(vote, weight, current_round))
    }

    fn result_invariant(
        &self,
        result: &Self::Result,
        expected_state: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        // Get expected result.
        let expected_result = &expected_state.last_emitted;
        println!(
            "🟣 result: model={:?}({:?},{:?}), code={:?}",
            expected_result.name, expected_result.value, expected_result.round, result
        );
        // Check result against expected result.
        match result {
            Some(result) => match result {
                Message::PolkaValue(value) => {
                    assert_eq!(expected_result.name, "PolkaValue");
                    assert_eq!(
                        value_from_model(&expected_result.value).as_ref(),
                        Some(value)
                    );
                }
                Message::PrecommitValue(value) => {
                    assert_eq!(expected_result.name, "PrecommitValue");
                    assert_eq!(
                        value_from_model(&expected_result.value).as_ref(),
                        Some(value)
                    );
                }
                Message::SkipRound(round) => {
                    assert_eq!(expected_result.name, "Skip");
                    assert_eq!(
                        &Round::new(from_itf(&expected_result.round).unwrap()),
                        round
                    );
                }
                msg => assert_eq!(expected_result.name, format!("{msg:?}")),
            },
            None => assert_eq!(expected_result.name, "None"),
        }
        Ok(true)
    }

    fn state_invariant(
        &self,
        _state: &Self::State,
        _expected_state: &Self::ExpectedState,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[fixture]
fn vote_keeper_runner() -> VoteKeeperRunner {
    let mut rng = StdRng::seed_from_u64(0x42);

    // build mapping from model addresses to real addresses
    VoteKeeperRunner {
        address_map: ADDRESSES
            .iter()
            .map(|&name| {
                let pk = PrivateKey::generate(&mut rng).public_key();
                (name.into(), Address::from_public_key(&pk))
            })
            .collect(),
    }
}

#[rstest]
fn test_itf(
    #[files("tests/fixtures/votekeeper/*.itf.json")] json_fixture: PathBuf,
    mut vote_keeper_runner: VoteKeeperRunner,
) {
    println!("Parsing {json_fixture:?}");

    let json = std::fs::read_to_string(&json_fixture).unwrap();
    let trace = itf::trace_from_str::<ItfState>(&json).unwrap();

    vote_keeper_runner
        .test(
            trace
                .states
                .into_iter()
                .map(|s| s.value)
                .collect::<Vec<_>>()
                .as_slice(),
        )
        .unwrap();
}
