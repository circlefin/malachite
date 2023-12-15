use malachite_itf::types::{NonNilValue, Value as ModelValue};
use malachite_test::{Value, ValueId};

pub const ADDRESSES: [&str; 2] = ["Josef", "Other"];

pub fn value_from_string(v: &NonNilValue) -> Option<Value> {
    match v.as_str() {
        "block" => Some(Value::new(1)),
        "nextBlock" => Some(Value::new(2)),
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
