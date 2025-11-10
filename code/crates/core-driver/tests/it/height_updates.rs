//! Tests for optional validator set and timeouts updates when starting/restarting heights

use std::time::Duration;

use malachitebft_core_types::{HeightUpdates, LinearTimeouts, Round};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Height, TestContext, ValidatorSet};

use informalsystems_malachitebft_core_driver::Driver;

/// Test that move_to_height preserves the existing validator set when HeightUpdates.validator_set is None
#[test]
fn move_to_height_preserves_validator_set_when_none() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let initial_timeouts = LinearTimeouts::default();

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set.clone(),
        initial_timeouts,
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.height(), initial_height);
    assert_eq!(driver.validator_set(), &initial_validator_set);

    // Move to next height with None validator_set - should preserve the existing one
    let next_height = Height::new(2);
    let updates = HeightUpdates {
        validator_set: None,
        timeouts: None,
    };

    driver.move_to_height(next_height, updates);

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Validator set should be unchanged
    assert_eq!(driver.validator_set(), &initial_validator_set);
}

/// Test that move_to_height updates the validator set when HeightUpdates.validator_set is Some
#[test]
fn move_to_height_updates_validator_set_when_some() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2.clone(), v3.clone()]);
    let initial_timeouts = LinearTimeouts::default();

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set.clone(),
        initial_timeouts,
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.validator_set(), &initial_validator_set);

    // Create a new validator set with different voting powers
    let [(new_v1, _), (new_v2, _), (new_v3, _)] = make_validators([5, 6, 7]);
    let new_validator_set = ValidatorSet::new(vec![new_v1, new_v2, new_v3]);

    // Move to next height with a new validator set
    let next_height = Height::new(2);
    let updates = HeightUpdates {
        validator_set: Some(new_validator_set.clone()),
        timeouts: None,
    };

    driver.move_to_height(next_height, updates);

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Validator set should be updated
    assert_eq!(driver.validator_set(), &new_validator_set);
    assert_ne!(driver.validator_set(), &initial_validator_set);
}

/// Test that move_to_height preserves the existing timeouts when HeightUpdates.timeouts is None
#[test]
fn move_to_height_preserves_timeouts_when_none() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1, v2, v3]);
    let initial_timeouts = LinearTimeouts {
        propose: Duration::from_secs(1),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(3),
    };

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set,
        initial_timeouts,
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.height(), initial_height);
    assert_eq!(driver.timeouts(), &initial_timeouts);

    // Move to next height with None timeouts - should preserve the existing ones
    let next_height = Height::new(2);
    let updates = HeightUpdates {
        validator_set: None,
        timeouts: None,
    };

    driver.move_to_height(next_height, updates);

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Timeouts should be unchanged
    assert_eq!(driver.timeouts(), &initial_timeouts);
}

/// Test that move_to_height updates the timeouts when HeightUpdates.timeouts is Some
#[test]
fn move_to_height_updates_timeouts_when_some() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1, v2, v3]);
    let initial_timeouts = LinearTimeouts {
        propose: Duration::from_secs(1),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(3),
    };

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set,
        initial_timeouts,
        my_addr,
        Default::default(),
    );

    assert_eq!(driver.timeouts(), &initial_timeouts);

    // Create new timeouts with different values
    let new_timeouts = LinearTimeouts {
        propose: Duration::from_secs(2),
        propose_delta: Duration::from_secs(1),
        prevote: Duration::from_secs(2),
        prevote_delta: Duration::from_secs(1),
        precommit: Duration::from_secs(2),
        precommit_delta: Duration::from_secs(1),
        rebroadcast: Duration::from_secs(6),
    };

    // Move to next height with new timeouts
    let next_height = Height::new(2);
    let updates = HeightUpdates {
        validator_set: None,
        timeouts: Some(new_timeouts),
    };

    driver.move_to_height(next_height, updates);

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Timeouts should be updated
    assert_eq!(driver.timeouts(), &new_timeouts);
    assert_ne!(driver.timeouts(), &initial_timeouts);
}

/// Test HeightUpdates::none() convenience method
#[test]
fn height_updates_none_creates_empty_updates() {
    let updates = HeightUpdates::<TestContext>::none();
    assert!(updates.validator_set.is_none());
    assert!(updates.timeouts.is_none());
}

/// Test that move_to_height with HeightUpdates::none() preserves everything
#[test]
fn move_to_height_with_none_preserves_all_state() {
    let [(v1, sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let (_my_sk, my_addr) = (sk1, v1.address);

    let initial_height = Height::new(1);
    let ctx = TestContext::new();
    let initial_validator_set = ValidatorSet::new(vec![v1, v2, v3]);
    let initial_timeouts = LinearTimeouts {
        propose: Duration::from_secs(1),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(3),
    };

    let mut driver = Driver::new(
        ctx,
        initial_height,
        initial_validator_set.clone(),
        initial_timeouts,
        my_addr,
        Default::default(),
    );

    // Move to next height with HeightUpdates::none()
    let next_height = Height::new(2);
    driver.move_to_height(next_height, HeightUpdates::none());

    assert_eq!(driver.height(), next_height);
    assert_eq!(driver.round(), Round::Nil);
    // Both validator set and timeouts should be preserved
    assert_eq!(driver.validator_set(), &initial_validator_set);
    assert_eq!(driver.timeouts(), &initial_timeouts);
}
