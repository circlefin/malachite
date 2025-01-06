use std::collections::HashMap;

use rand::{CryptoRng, RngCore};

use malachitebft_core_types::NilOrVal;
use malachitebft_core_votekeeper as core;
use malachitebft_test::{
    Address, PrivateKey, PublicKey, TestContext, Validator, ValidatorSet, ValueId,
};

use crate::types::Value;

pub const VALIDATORS: [&str; 3] = ["alice", "bob", "john"];

pub fn build_address_map<'a>(
    public_keys: impl Iterator<Item = (&'a String, &'a PublicKey)>,
) -> HashMap<String, Address> {
    public_keys
        .map(|(name, pk)| (name.clone(), Address::from_public_key(pk)))
        .collect()
}

pub fn build_validator_set<'a, R>(
    weights: impl Iterator<Item = (&'a String, &'a i64)>,
    mut rng: R,
) -> ValidatorSet
where
    R: RngCore + CryptoRng,
{
    let validators = weights.map(|(_name, weight)| {
        let public_key = PrivateKey::generate(&mut rng).public_key();
        Validator::new(public_key, *weight as u64)
    });

    ValidatorSet::new(validators)
}

pub fn value_from_model(value: &Value) -> NilOrVal<ValueId> {
    match value {
        Value::Nil => NilOrVal::Nil,
        Value::Val(v) => match v.as_str() {
            "v1" => NilOrVal::Val(1.into()),
            "v2" => NilOrVal::Val(2.into()),
            "v3" => NilOrVal::Val(3.into()),
            _ => unimplemented!("unknown value {value:?}"),
        },
    }
}

pub fn check_votes(
    expected: &crate::votekeeper::VoteCount,
    actual: &core::count::VoteCount<TestContext>,
    address_map: &HashMap<String, Address>,
) {
    // expected has `total_weight` which is not present in actual

    let expected_values_weights = &expected.values_weights;
    let actual_values_weights = &actual.values_weights;

    // should check length too

    for value in expected_values_weights.keys() {
        assert_eq!(
            actual_values_weights.get(&value_from_model(value)),
            *expected_values_weights.get(value).unwrap() as u64,
            "weight for value {value:?}"
        );
    }

    let expected_votes_addresses = &expected.votes_addresses;
    let actual_votes_addresses = &actual.validator_addresses;

    assert_eq!(
        actual_votes_addresses.len(),
        expected_votes_addresses.len(),
        "number of voted addresses"
    );

    for address in expected_votes_addresses {
        assert!(
            actual_votes_addresses.contains(address_map.get(address).unwrap()),
            "address {address:?} not voted"
        );
    }
}
