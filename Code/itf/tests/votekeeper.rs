use std::collections::HashMap;
use std::path::PathBuf;

use rand::rngs::StdRng;
use rand::SeedableRng;

use malachite_common::Round;
use malachite_itf::votekeeper::State as ItfState;
use malachite_itf::votekeeper::Value as ItfValue;
use malachite_test::{Address, PrivateKey, TestContext, ValueId, Vote};
use malachite_vote::keeper::{Message, VoteKeeper};
use malachite_vote::Weight;

use itf::Itf;
use rstest::{fixture, rstest};

// TODO: move to itf-rs repo
fn from_itf<T, U>(bigint: Itf<T>) -> Option<U>
where
    U: TryFrom<T>,
{
    bigint.value().try_into().ok()
}

fn value_from_model(value: ItfValue) -> Option<ValueId> {
    match value.as_str() {
        "nil" => None,
        "proposal" => Some(0.into()),
        "val1" => Some(1.into()),
        "val2" => Some(2.into()),
        "val3" => Some(3.into()),
        _ => unimplemented!("unknown value {value:?}"),
    }
}

#[fixture]
#[once]
fn model_address_map() -> HashMap<String, Address> {
    let mut rng = StdRng::seed_from_u64(0x42);

    // build mapping from model addresses to real addresses
    let valid_model_addresses = ["alice", "bob", "john"];
    valid_model_addresses
        .iter()
        .map(|&name| {
            let pk = PrivateKey::generate(&mut rng).public_key();
            (name.into(), Address::from_public_key(&pk))
        })
        .collect()
}

#[rstest]
fn test_itf(
    #[files("tests/fixtures/votekeeper/*.json")] json_fixture: PathBuf,
    model_address_map: &HashMap<String, Address>,
) {
    println!("Parsing {json_fixture:?}");

    let json = std::fs::read_to_string(&json_fixture).unwrap();
    let trace = itf::trace_from_str::<ItfState>(&json).unwrap();

    // Obtain the initial total_weight from the first state in the model.
    let bookkeper = trace.states[0].value.bookkeeper.clone();
    let total_weight: Weight = from_itf(bookkeper.total_weight).unwrap();

    let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(total_weight);

    for state in &trace.states[1..] {
        let state = state.clone().value;

        // Build step to execute.
        let (input_vote, weight) = state.weighted_vote.value();
        let round = Round::new(from_itf(input_vote.round).unwrap());
        let value = value_from_model(input_vote.value);
        let address = model_address_map.get(input_vote.address.as_str()).unwrap();
        let vote = match input_vote.typ.as_str() {
            "Prevote" => Vote::new_prevote(round, value, *address),
            "Precommit" => Vote::new_precommit(round, value, *address),
            _ => unreachable!(),
        };
        let weight: Weight = from_itf(weight).unwrap();
        println!(
            "🟢 step: vote={:?}, round={:?}, value={:?}, address={:?}, weight={:?}",
            input_vote.typ, round, value, input_vote.address, weight
        );

        // Execute step.
        let result = keeper.apply_vote(vote.clone(), weight);

        // Get expected result.
        let model_result = state.last_emitted;
        println!(
            "🟣 result: model={:?}({:?}), code={:?}",
            model_result.name, model_result.value, result
        );

        // Check result against expected result.
        match result {
            Some(result) => match result {
                Message::PolkaValue(value) => {
                    assert_eq!(model_result.name, "PolkaValue");
                    assert_eq!(value_from_model(model_result.value), Some(value));
                }
                Message::PrecommitValue(value) => {
                    assert_eq!(model_result.name, "PrecommitValue");
                    assert_eq!(value_from_model(model_result.value), Some(value));
                }
                Message::SkipRound(round) => {
                    assert_eq!(model_result.name, "SkipRound");
                    assert_eq!(Round::new(from_itf(model_result.round).unwrap()), round);
                }
                msg => assert_eq!(model_result.name, format!("{:?}", msg)),
            },
            None => assert_eq!(model_result.name, "None"),
        }
    }
}
