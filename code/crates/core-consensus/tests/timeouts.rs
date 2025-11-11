//! Tests for timeout state management in core-consensus

use std::time::Duration;

use informalsystems_malachitebft_core_consensus::{Params, State};
use malachitebft_core_types::{HeightUpdates, LinearTimeouts, Round, TimeoutKind, ValuePayload};
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Height, TestContext, ValidatorSet};

/// Test that reset_and_start_height preserves timeouts when HeightUpdates.timeouts is None
#[test]
fn reset_and_start_height_preserves_timeouts_when_none() {
    let ctx = TestContext::new();
    let [(v1, _sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2, v3]);

    let initial_timeouts = LinearTimeouts {
        propose: Duration::from_secs(1),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(3),
    };

    let params = Params {
        initial_height: Height::new(1),
        initial_validator_set,
        initial_timeouts,
        address: v1.address,
        threshold_params: Default::default(),
        value_payload: ValuePayload::ProposalAndParts,
        enabled: true,
    };

    let mut state = State::new(ctx, params, 100);

    assert_eq!(state.height(), Height::new(1));
    assert_eq!(state.timeouts(), &initial_timeouts);

    // Move to next height with None timeouts - should preserve the existing ones
    let next_height = Height::new(2);
    let updates = HeightUpdates::default();

    state.reset_and_start_height(next_height, updates);

    assert_eq!(state.height(), next_height);
    assert_eq!(state.round(), Round::Nil);
    // Timeouts should be unchanged
    assert_eq!(state.timeouts(), &initial_timeouts);
}

/// Test that reset_and_start_height updates timeouts when HeightUpdates.timeouts is Some
#[test]
fn reset_and_start_height_updates_timeouts_when_some() {
    let ctx = TestContext::new();
    let [(v1, _sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2, v3]);

    let initial_timeouts = LinearTimeouts {
        propose: Duration::from_secs(1),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(3),
    };

    let params = Params {
        initial_height: Height::new(1),
        initial_validator_set,
        initial_timeouts,
        address: v1.address,
        threshold_params: Default::default(),
        value_payload: ValuePayload::ProposalAndParts,
        enabled: true,
    };

    let mut state = State::new(ctx, params, 100);

    assert_eq!(state.timeouts(), &initial_timeouts);

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
    let updates = HeightUpdates::default().with_timeouts(new_timeouts);

    state.reset_and_start_height(next_height, updates);

    assert_eq!(state.height(), next_height);
    assert_eq!(state.round(), Round::Nil);
    // Timeouts should be updated
    assert_eq!(state.timeouts(), &new_timeouts);
    assert_ne!(state.timeouts(), &initial_timeouts);
}

/// Test that timeouts can be used to calculate durations for different timeout kinds
#[test]
fn timeouts_can_calculate_durations() {
    let ctx = TestContext::new();
    let [(v1, _sk1), (v2, _sk2), (v3, _sk3)] = make_validators([1, 2, 3]);
    let initial_validator_set = ValidatorSet::new(vec![v1.clone(), v2, v3]);

    let timeouts = LinearTimeouts {
        propose: Duration::from_secs(3),
        propose_delta: Duration::from_millis(500),
        prevote: Duration::from_secs(1),
        prevote_delta: Duration::from_millis(500),
        precommit: Duration::from_secs(1),
        precommit_delta: Duration::from_millis(500),
        rebroadcast: Duration::from_secs(5),
    };

    let params = Params {
        initial_height: Height::new(1),
        initial_validator_set,
        initial_timeouts: timeouts,
        address: v1.address,
        threshold_params: Default::default(),
        value_payload: ValuePayload::ProposalAndParts,
        enabled: true,
    };

    let state = State::new(ctx, params, 100);

    // Test that we can calculate durations for different rounds and timeout kinds
    assert_eq!(
        state
            .timeouts()
            .duration_for(TimeoutKind::Propose, Round::new(0)),
        Duration::from_secs(3)
    );
    assert_eq!(
        state
            .timeouts()
            .duration_for(TimeoutKind::Propose, Round::new(1)),
        Duration::from_millis(3500)
    );
    assert_eq!(
        state
            .timeouts()
            .duration_for(TimeoutKind::Prevote, Round::new(0)),
        Duration::from_secs(1)
    );
    assert_eq!(
        state
            .timeouts()
            .duration_for(TimeoutKind::Precommit, Round::new(0)),
        Duration::from_secs(1)
    );
    assert_eq!(
        state
            .timeouts()
            .duration_for(TimeoutKind::Rebroadcast, Round::new(0)),
        Duration::from_secs(5)
    );
}
