#![allow(clippy::needless_update)]

use informalsystems_malachitebft_core_consensus::{process, Effect, Error, Params, Resumable, Resume, State, WalEntry};
use malachitebft_core_types::{Round, ThresholdParams, ValuePayload};
use malachitebft_metrics::Metrics;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Height, TestContext, ValidatorSet};

use super::utils::{propose_timeout, wal_entry_to_input};

/// Minimal harness for driving core-consensus deterministically.
///
/// This is intentionally small and does **not** attempt to simulate full networking or timing.
/// It records WAL entries emitted via `Effect::WalAppend` and can replay them after a simulated crash.
struct Harness {
    state: State<TestContext>,
    // In-memory WAL: (height, entry)
    wal: Vec<(Height, WalEntry<TestContext>)>,
}

impl Harness {
    fn new(height: Height, vs: ValidatorSet) -> Self {
        let ctx = TestContext::new();
        let params = Params {
            address: vs
                .get_by_index(0)
                .expect("validator set must be non-empty")
                .address,
            threshold_params: ThresholdParams::default(),
            value_payload: ValuePayload::ProposalOnly,
            enabled: true,
        };

        Self {
            state: State::new(ctx, height, vs, params, 128),
            wal: Vec::new(),
        }
    }

    fn run(&mut self, input: informalsystems_malachitebft_core_consensus::Input<TestContext>) {
        let metrics = Metrics::new();

        // Split borrows so the effect handler doesn't need to borrow `self` while `state` is mutably borrowed.
        let state = &mut self.state;
        let wal = &mut self.wal;

        // Metrics expects step_start/step_end pairing; initialize it to the current driver step.
        metrics.step_start(state.driver.step());

        let _res: Result<(), informalsystems_malachitebft_core_consensus::Error<TestContext>> =
            process!(
                input: input,
                state: state,
                metrics: &metrics,
                with: effect => {
                    let res: Result<Resume<TestContext>, Error<TestContext>> = match effect {
                            Effect::WalAppend(height, entry, r) => {
                                wal.push((height, entry));
                                Ok(r.resume_with(()))
                            }
                            // For this PR we keep the effect handler conservative: always continue.
                            // Follow-up PRs will add specific effect simulation (signing, publishing, etc.).
                            other => {
                                let _ = other;
                                Ok(Resume::Continue)
                            }
                        };
                    res
                }
            );

        let _ = _res;
    }

    fn drain_wal_entries(&self, height: Height) -> Vec<WalEntry<TestContext>> {
        self.wal
            .iter()
            .filter_map(|(h, e)| (*h == height).then(|| e.clone()))
            .collect()
    }
}

#[test]
fn wal_entries_can_be_captured_and_replayed_in_memory() {
    let [(v1, _sk1), (v2, _sk2), (v3, _sk3), (v4, _sk4)] = make_validators([1, 1, 1, 1]);
    let vs = ValidatorSet::new(vec![v1, v2, v3, v4]);

    // First run: start height and trigger a persisted timeout.
    let mut h1 = Harness::new(Height::new(1), vs.clone());

    h1.run(informalsystems_malachitebft_core_consensus::Input::StartHeight(
        Height::new(1),
        vs.clone(),
        false,
    ));

    h1.run(informalsystems_malachitebft_core_consensus::Input::TimeoutElapsed(
        propose_timeout(0),
    ));

    let wal_entries = h1.drain_wal_entries(Height::new(1));
    assert!(
        wal_entries.iter().any(|e| matches!(e, WalEntry::Timeout(t) if t.round == Round::new(0))),
        "expected a timeout WAL entry for round 0"
    );

    // Simulated crash/restart: new harness, replay WAL entries as inputs.
    let vs2 = vs;
    let mut h2 = Harness::new(Height::new(1), vs2.clone());
    h2.run(informalsystems_malachitebft_core_consensus::Input::StartHeight(
        Height::new(1),
        vs2,
        true,
    ));

    for entry in wal_entries {
        h2.run(wal_entry_to_input(entry));
    }

    // After replay, we should have persisted the same timeout again deterministically.
    let wal_entries_2 = h2.drain_wal_entries(Height::new(1));
    assert!(
        wal_entries_2.iter().any(|e| matches!(e, WalEntry::Timeout(t) if t.round == Round::new(0))),
        "expected timeout WAL entry after replay"
    );
}
