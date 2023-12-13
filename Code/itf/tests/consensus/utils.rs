use malachite_itf::types::{NonNilValue, Value as ModelValue};
use malachite_test::{Value, ValueId};

pub const ADDRESSES: [&str; 3] = ["Alice", "Bob", "Josef"];

pub fn value_from_string(v: &NonNilValue) -> Option<Value> {
    match v.as_str() {
        "val1" => Some(Value::new(0)),
        "val2" => Some(Value::new(1)),
        "val3" => Some(Value::new(2)),
        _ => unimplemented!("unknown value {v:?}"),
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
