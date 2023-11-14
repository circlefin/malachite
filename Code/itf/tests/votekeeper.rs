use std::collections::HashMap;

use itf::ItfBigInt;
use malachite_common::Round;
use malachite_itf::votekeeper::State;
use malachite_test::{Address, PrivateKey, TestContext, ValueId, Vote};
use malachite_vote::keeper::Message;
use malachite_vote::{keeper::VoteKeeper, Weight};
use num_bigint::Sign;

use rand::rngs::StdRng;
use rand::SeedableRng;

#[test]
fn parse_fixtures() {
    let mut rng = StdRng::seed_from_u64(0x42);

    // build mapping from model addresses to real addresses
    let valid_model_addresses = vec!["alice", "bob", "john"];
    let model_address_map: HashMap<&str, Address> = valid_model_addresses
        .iter()
        .map(|&name| {
            let pk = PrivateKey::generate(&mut rng).public_key();
            (name, Address::from_public_key(&pk))
        })
        .collect();

    // read fixtures files in test/fixtures/votekeeper/
    let folder = format!("{}/tests/fixtures/votekeeper", env!("CARGO_MANIFEST_DIR"));

    let fixtures = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().map_or(false, |ext| ext == "json"))
        .collect::<Vec<_>>();

    for fixture in fixtures {
        println!("Parsing '{}'", fixture.display());

        let json = std::fs::read_to_string(&fixture).unwrap();
        let trace = itf::trace_from_str::<State>(&json).unwrap();

        // Obtain the initial total_weight from the first state in the model.
        let bookkeper = trace.states[0].value.bookkeeper.clone();
        let total_weight: Weight = uint_from_model(bookkeper.total_weight).unwrap();

        let mut keeper: VoteKeeper<TestContext> = VoteKeeper::new(total_weight);

        for state in &trace.states[1..] {
            let state = state.clone().value;

            // Build step to execute.
            let (input_vote, weight) = state.weighted_vote.value();
            let round = round_from_model(input_vote.round);
            let value = value_from_model(input_vote.value);
            let address = model_address_map.get(input_vote.address.as_str()).unwrap();
            let vote = match input_vote.typ.as_str() {
                "Prevote" => Vote::new_prevote(round, value, *address),
                "Precommit" => Vote::new_precommit(round, value, *address),
                _ => unreachable!(),
            };
            let weight: Weight = uint_from_model(weight).unwrap();
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
            if result.is_none() {
                assert_eq!(model_result.name, "None");
            } else {
                match result.unwrap() {
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
                        assert_eq!(round_from_model(model_result.round), round);
                    }
                    msg => assert_eq!(model_result.name, format!("{:?}", msg)),
                }
            }
        }
    }
}

// TODO: move to itf-rs repo
fn uint_from_model(bigint: ItfBigInt) -> Option<u64> {
    let (sign, digits) = bigint.value().to_u64_digits();
    if sign == Sign::Minus {
        None
    } else {
        Some(u64::try_from(digits[0]).unwrap())
    }
}

// TODO: move to itf-rs repo
fn extract_int(bigint: ItfBigInt) -> Option<i64> {
    let (sign, digits) = bigint.value().to_u64_digits();
    let i = i64::try_from(digits[0]).unwrap();
    Some(if sign == Sign::Minus { -i } else { i })
}

fn round_from_model(round: malachite_itf::votekeeper::Round) -> Round {
    let i = extract_int(round).unwrap();
    if i < 0 {
        Round::Nil
    } else {
        Round::Some(i)
    }
}

fn value_from_model(value: malachite_itf::votekeeper::Value) -> Option<malachite_test::ValueId> {
    match value.as_str() {
        "nil" => None,
        "proposal" => Some(ValueId::from(0)),
        "val1" => Some(ValueId::from(1)),
        "val2" => Some(ValueId::from(2)),
        "val3" => Some(ValueId::from(3)),
        _ => None,
    }
}
