//! Tests for optional validator set updates when starting/restarting heights

use malachitebft_core_types::{HeightUpdates, Round};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Height, TestContext, ValidatorSet};

use informalsystems_malachitebft_core_driver::Driver;

/// Test that move_to_height preserves the existing validator set when validator_set is None
#[test]
fn move_to_height_preserves_validator_set_when_none() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set.clone(),
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.height(), initial_height);
    assert_eq!(driver.validator_set(), &initial_validator_set);

    // Move to next height with None validator_set - should preserve the existing one
    let next_height = Height::new(2);

    driver.move_to_height(next_height, None);

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Validator set should be unchanged
    assert_eq!(driver.validator_set(), &initial_validator_set);
}

/// Test that move_to_height updates the validator set when validator_set is Some
#[test]
fn move_to_height_updates_validator_set_when_some() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set.clone(),
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.validator_set(), &initial_validator_set);

    // Create a new validator set with different voting powers
    let [(new_v1, _), (new_v2, _), (new_v3, _)] = make_validators([5, 6, 7]);
    let new_validator_set = ValidatorSet::new(vec![new_v1, new_v2, new_v3]);

    // Move to next height with a new validator set
    let next_height = Height::new(2);
    driver.move_to_height(next_height, Some(new_validator_set.clone()));

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Validator set should be updated
    assert_eq!(driver.validator_set(), &new_validator_set);
    assert_ne!(driver.validator_set(), &initial_validator_set);
}

/// Test HeightUpdates::default() convenience method
#[test]
fn height_updates_default_creates_empty_updates() {
    let updates = HeightUpdates::<TestContext>::default();
    assert!(updates.validator_set.is_none());
    assert!(updates.timeouts.is_none());
}
