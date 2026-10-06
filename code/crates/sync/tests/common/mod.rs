//! Shared harness for the sync integration tests: a driver for the
//! coroutine-based `handle`, plus state and value constructors.
//!
//! Cargo compiles this module into each test binary separately, so a helper
//! only one binary uses looks dead to the others.
#![allow(dead_code)]

use arc_malachitebft_sync::co::{CoState, Gen};
use arc_malachitebft_sync::handle::{self, Input};
use arc_malachitebft_sync::{
    Config, Effect, Error, Metrics, OutboundRequestId, RawDecidedValue, Resume, State,
};
use arc_malachitebft_test::{Height, TestContext, ValueId};
use bytes::Bytes;
use malachitebft_core_types::{ExtendedCommitCertificate, Round};
use rand::SeedableRng;

/// A state with a fixed RNG seed, so peer selection is reproducible.
pub fn make_test_state_with(config: Config) -> State<TestContext> {
    State::new(Box::new(rand::rngs::StdRng::seed_from_u64(42)), config)
}

pub fn make_test_state() -> State<TestContext> {
    make_test_state_with(Config::default())
}

/// Feed one input to `handle` and collect the effects it yields. `resume`
/// produces the value fed back for each effect.
fn drive_input_with(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    input: Input<TestContext>,
    mut resume: impl FnMut(&Effect<TestContext>) -> Resume<TestContext>,
) -> Result<Vec<Effect<TestContext>>, Error<TestContext>> {
    let mut effects = Vec::new();
    let mut gen = Gen::new(|co| handle::handle(co, state, metrics, input));
    let mut result = gen.resume_with(Resume::default());

    loop {
        match result {
            CoState::Yielded(effect) => {
                let next = resume(&effect);
                effects.push(effect);
                result = gen.resume_with(next);
            }
            CoState::Complete(r) => return r.map(|()| effects),
        }
    }
}

/// Resume every effect with the default. Only safe for inputs whose handling
/// does not need a meaningful resume value.
pub fn drive_input(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    input: Input<TestContext>,
) -> Result<Vec<Effect<TestContext>>, Error<TestContext>> {
    drive_input_with(state, metrics, input, |_| Resume::default())
}

/// Resume each `SendValueRequest` with `{prefix}{n}`, counting from
/// `*next_id + 1`, so the caller can keep one id sequence across several
/// calls and name the ids in its assertions.
pub fn drive_input_numbering_requests(
    state: &mut State<TestContext>,
    metrics: &Metrics,
    input: Input<TestContext>,
    prefix: &str,
    next_id: &mut u64,
) -> Result<Vec<Effect<TestContext>>, Error<TestContext>> {
    drive_input_with(state, metrics, input, |effect| match effect {
        Effect::SendValueRequest(..) => {
            *next_id += 1;
            Resume::ValueRequestId(Some(OutboundRequestId::new(format!("{prefix}{next_id}"))))
        }
        _ => Resume::default(),
    })
}

pub fn make_raw_value(height: u64) -> RawDecidedValue<TestContext> {
    RawDecidedValue::new(
        Bytes::from_static(b"test"),
        ExtendedCommitCertificate {
            height: Height::new(height),
            round: Round::ZERO,
            value_id: ValueId::new(height),
            commit_signatures: vec![],
        },
    )
}
