//! Integration tests for [`Amnesia<Ctx>`].
//!
//! These tests exercise the amnesia state machine through the
//! context-generic API (`record_proposed_value`, `try_override_nil_prevote`,
//! `should_ignore_locks`). The helpers are written as generic functions over
//! `Ctx: Context` so the test file cannot accidentally rely on `TestContext`
//! type specifics beyond trait bounds — any context-coupling slip would show
//! up as a compile error on the generic helpers.
//!
//! `TestContext` is used as the concrete context only to obtain the
//! associated-type instances (heights, value ids) the helpers operate on.

use arc_malachitebft_engine_byzantine::{Amnesia, Trigger};
use malachitebft_core_types::{Context, Round, ValueId};
use malachitebft_test::{Height, TestContext, Value};

/// Generic helper: records a value, then asserts an override at matching
/// `(height, round)`. Any `Ctx: Context` works here — the test compiles only
/// because `Amnesia<Ctx>`'s API is context-generic.
fn assert_records_and_overrides<Ctx: Context>(
    amnesia: &Amnesia<Ctx>,
    height: Ctx::Height,
    round: Round,
    value_id: ValueId<Ctx>,
) {
    amnesia.record_proposed_value(height, round, value_id.clone());

    let overridden = amnesia
        .try_override_nil_prevote(height, round)
        .expect("trigger fires at matching (h, r) => override");

    assert_eq!(overridden, value_id, "override returns the recorded value");

    // Cached value is consumed on override — a second call is a miss.
    assert!(
        amnesia.try_override_nil_prevote(height, round).is_none(),
        "cache is consumed on override"
    );
}

/// Generic helper: records a value for `(h1, r1)` but asks for an override
/// at a different `(h2, r2)`. The mismatch must not override.
fn assert_mismatch_no_override<Ctx: Context>(
    amnesia: &Amnesia<Ctx>,
    recorded_at: (Ctx::Height, Round),
    asked_at: (Ctx::Height, Round),
    value_id: ValueId<Ctx>,
) {
    amnesia.record_proposed_value(recorded_at.0, recorded_at.1, value_id);
    assert!(
        amnesia
            .try_override_nil_prevote(asked_at.0, asked_at.1)
            .is_none(),
        "mismatched (h, r) must not override"
    );
}

#[test]
fn records_and_overrides_on_match() {
    let amnesia = Amnesia::<TestContext>::new(Trigger::Always, Some(0));
    let height = Height::new(7);
    let round = Round::new(2);
    let value_id = Value::new(42).id();
    assert_records_and_overrides(&amnesia, height, round, value_id);
}

#[test]
fn does_not_override_when_height_mismatches() {
    let amnesia = Amnesia::<TestContext>::new(Trigger::Always, Some(0));
    let recorded_at = (Height::new(7), Round::new(2));
    let asked_at = (Height::new(8), Round::new(2));
    let value_id = Value::new(42).id();
    assert_mismatch_no_override(&amnesia, recorded_at, asked_at, value_id);
}

#[test]
fn does_not_override_when_round_mismatches() {
    let amnesia = Amnesia::<TestContext>::new(Trigger::Always, Some(0));
    let recorded_at = (Height::new(7), Round::new(2));
    let asked_at = (Height::new(7), Round::new(3));
    let value_id = Value::new(42).id();
    assert_mismatch_no_override(&amnesia, recorded_at, asked_at, value_id);
}

#[test]
fn trigger_never_short_circuits() {
    let amnesia = Amnesia::<TestContext>::new(Trigger::Never, Some(0));

    let height = Height::new(1);
    let round = Round::new(0);
    amnesia.record_proposed_value(height, round, Value::new(42).id());

    // `Trigger::Never` means `should_ignore_locks` returns false regardless of
    // any cached value, so `try_override_nil_prevote` must be None.
    assert!(!amnesia.should_ignore_locks(height, round));
    assert!(amnesia.try_override_nil_prevote(height, round).is_none());
}

#[test]
fn no_cached_value_returns_none() {
    let amnesia = Amnesia::<TestContext>::new(Trigger::Always, Some(0));
    let height = Height::new(1);
    let round = Round::new(0);
    assert!(amnesia.try_override_nil_prevote(height, round).is_none());
}

#[test]
fn at_heights_trigger_only_fires_at_configured_heights() {
    let amnesia = Amnesia::<TestContext>::new(
        Trigger::AtHeights {
            heights: vec![3, 7],
        },
        Some(0),
    );

    let value_id = Value::new(42).id();

    // Height not in the set — the trigger doesn't fire, so the override
    // short-circuits even though the value is cached.
    amnesia.record_proposed_value(Height::new(5), Round::new(0), value_id);
    assert!(amnesia
        .try_override_nil_prevote(Height::new(5), Round::new(0))
        .is_none());

    // Height in the set — override fires.
    amnesia.record_proposed_value(Height::new(3), Round::new(0), value_id);
    let overridden = amnesia
        .try_override_nil_prevote(Height::new(3), Round::new(0))
        .expect("AtHeights should fire at configured heights");
    assert_eq!(overridden, value_id);
}
