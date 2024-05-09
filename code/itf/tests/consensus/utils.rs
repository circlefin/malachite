use std::collections::BTreeMap;

use malachite_common::Transaction;
use malachite_itf::consensus::State;
use malachite_itf::types::{NonNilValue, Value as ModelValue};
use malachite_test::{Address, PrivateKey, Value, ValueId};
use rand::rngs::StdRng;

pub const OTHER_PROCESS: &str = "Other";

/// Build mapping from model addresses to real addresses
pub fn build_address_map(trace: &itf::Trace<State>, rng: &mut StdRng) -> BTreeMap<String, Address> {
    trace
        .states
        .iter()
        .map(|state| state.value.state.process.clone())
        .chain(std::iter::once(OTHER_PROCESS.to_string()))
        .map(|name| {
            let pk = PrivateKey::generate(&mut *rng).public_key();
            (name, Address::from_public_key(&pk))
        })
        .collect()
}

pub fn value_from_string(v: &NonNilValue) -> Option<Value> {
    //let value1 = Value::new([Transaction(Vec::from(1_i32.to_be_bytes()))].to_vec());
    let value2 = Value::new([Transaction(Vec::from(2_i32.to_be_bytes()))].to_vec());
    let value3 = Value::new([Transaction("block".as_bytes().to_vec())].to_vec());

    match v.as_str() {
        "block" => Some(value3),
        "nextBlock" => Some(value2),
        _ => panic!("unknown value {v:?}"),
    }
}

pub fn value_from_model(value: &ModelValue) -> Option<Value> {
    match value {
        ModelValue::Nil => None,
        ModelValue::Val(v) => value_from_string(v),
    }
}

pub fn value_id_from_model(value: &ModelValue) -> Option<ValueId> {
    value_from_model(value).map(|v| v.id())
}

pub fn value_id_from_string(v: &NonNilValue) -> Option<ValueId> {
    value_from_string(v).map(|v| v.id())
}
