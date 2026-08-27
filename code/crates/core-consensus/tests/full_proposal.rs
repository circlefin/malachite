//! `FullProposalKeeper`: pairing application `ProposedValue`s (payload + validity) with signed
//! `Proposal`s.
//!
//! Test layout:
//! - **BASIC** — `pol_round` is always nil (`-1`). Covers same vs different rounds, same vs
//!   different value ids, message order, and validity on the proposed value.
//! - **POL** — at least one proposal uses a non-nil `pol_round` (proof-of-lock / L28-style mux).

use futures::executor::block_on;
use malachitebft_core_types::{Round, SignedProposal, Validity, ValueOrigin};
use malachitebft_signing::Signer;
use malachitebft_test::utils::validators::make_validators;
use malachitebft_test::{Address, Ed25519Signer, Proposal, Value};
use malachitebft_test::{Height, TestContext};

use arc_malachitebft_core_consensus::full_proposal::{
    FullProposal, FullProposalKeeper, StoreProposalResult,
};
use arc_malachitebft_core_consensus::{Input, ProposedValue};

fn signed_proposal_at(
    signer: &Ed25519Signer,
    height: Height,
    round: Round,
    value: Value,
    pol_round: Round,
    address: Address,
) -> SignedProposal<TestContext> {
    let proposal = Proposal::new(height, round, value, pol_round, address);
    block_on(signer.sign_proposal(proposal)).unwrap()
}

/// Signed proposal at height 1.
fn signed_proposal(
    signer: &Ed25519Signer,
    address: Address,
    round: u32,
    value: u64,
    pol_round: i64,
) -> SignedProposal<TestContext> {
    signed_proposal_at(
        signer,
        Height::new(1),
        Round::new(round),
        Value::new(value),
        Round::from(pol_round),
        address,
    )
}

fn proposal_input(
    signer: &Ed25519Signer,
    address: Address,
    round: u32,
    value: u64,
    pol_round: i64,
) -> Input<TestContext> {
    Input::Proposal(signed_proposal(signer, address, round, value, pol_round))
}

fn proposed_value(
    proposer: Address,
    round: u32,
    value: u64,
    validity: Validity,
) -> ProposedValue<TestContext> {
    ProposedValue {
        height: Height::new(1),
        round: Round::new(round),
        valid_round: Round::Nil,
        proposer,
        value: Value::new(value),
        validity,
    }
}

fn value_input(
    proposer: Address,
    round: u32,
    value: u64,
    validity: Validity,
) -> Input<TestContext> {
    Input::ProposedValue(
        proposed_value(proposer, round, value, validity),
        ValueOrigin::Consensus,
    )
}

fn full_proposal_at(
    keeper: &FullProposalKeeper<TestContext>,
    round: u32,
    value: u64,
) -> Option<&FullProposal<TestContext>> {
    keeper.full_proposal_at_round_and_value(
        &Height::new(1),
        Round::new(round),
        &Value::new(value).id(),
    )
}

fn proposals_for_proposed_value(
    keeper: &FullProposalKeeper<TestContext>,
    pv: &ProposedValue<TestContext>,
) -> Vec<SignedProposal<TestContext>> {
    keeper.proposals_for_value(pv)
}

struct Case {
    /// Human-readable label (printed while the test runs).
    name: &'static str,
    /// Messages (`Proposal` or `ProposedValue`) applied to the keeper in order.
    input: Vec<Input<TestContext>>,
    /// For each `(round, value_id)`, assert `full_proposal_at(round, value_id).is_some()`.
    expect_full_for: Vec<(u32, u64)>,
    /// For each `(round, value_id)`, assert `full_proposal_at(round, value_id).is_none()`.
    expect_not_full_for: Vec<(u32, u64)>,
    /// After processing `input`, assert `keeper.proposals_for_value(&proposed_value)` equals the
    /// given proposal list. Only **`Full`** entries for that value id contribute.
    proposals_for: (ProposedValue<TestContext>, Vec<SignedProposal<TestContext>>),
}

#[test]
fn full_proposal_keeper_tests() {
    let [(v1, sk1), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let a2 = v2.address;
    let c1 = Ed25519Signer::new(sk1);
    let c2 = Ed25519Signer::new(sk2);

    let cases = vec![
        // --- BASIC (pol_round nil) ---
        Case {
            name: "BASIC: proposal r0 then value r0 same id — Full",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 10, Validity::Valid),
            ],
            expect_full_for: vec![(0, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![signed_proposal(&c1, a1, 0, 10, -1)],
            ),
        },
        Case {
            name: "BASIC: value r0 then proposal r0 same id — Full",
            input: vec![
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c1, a1, 0, 10, -1),
            ],
            expect_full_for: vec![(0, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![signed_proposal(&c1, a1, 0, 10, -1)],
            ),
        },
        Case {
            name: "BASIC: proposal r0 then value r0 same id invalid — still Full",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 10, Validity::Invalid),
            ],
            expect_full_for: vec![(0, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Invalid),
                vec![signed_proposal(&c1, a1, 0, 10, -1)],
            ),
        },
        Case {
            name: "BASIC: proposal id 10 then value id 20 same round — no Full",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 20, Validity::Valid),
            ],
            expect_full_for: vec![],
            expect_not_full_for: vec![(0, 10), (0, 20)],
            proposals_for: (proposed_value(a1, 0, 20, Validity::Valid), vec![]),
        },
        Case {
            name: "BASIC: two proposals r0 (10 then 20) then value 20 — Full only for 20",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                proposal_input(&c1, a1, 0, 20, -1),
                value_input(a1, 0, 20, Validity::Valid),
            ],
            expect_full_for: vec![(0, 20)],
            expect_not_full_for: vec![(0, 10)],
            proposals_for: (
                proposed_value(a1, 0, 20, Validity::Valid),
                vec![signed_proposal(&c1, a1, 0, 20, -1)],
            ),
        },
        Case {
            name: "BASIC: interleaved two ids r0 — both Full",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 20, Validity::Valid),
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c1, a1, 0, 20, -1),
            ],
            expect_full_for: vec![(0, 10), (0, 20)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![signed_proposal(&c1, a1, 0, 10, -1)],
            ),
        },
        Case {
            name: "BASIC: value r0 id 10 then proposal r2 id 10 nil pol — cross-round Full",
            input: vec![
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c1, a1, 2, 10, -1),
            ],
            expect_full_for: vec![(2, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![signed_proposal(&c1, a1, 2, 10, -1)],
            ),
        },
        // --- POL (non-nil pol_round) ---
        Case {
            name: "POL: r0 original then r1 re-propose same value pol=0 — two Full",
            input: vec![
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c2, a2, 1, 10, 0),
            ],
            expect_full_for: vec![(0, 10), (1, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![
                    signed_proposal(&c1, a1, 0, 10, -1),
                    signed_proposal(&c2, a2, 1, 10, 0),
                ],
            ),
        },
        Case {
            name:
                "POL: r1 pol before r0; then value 20 while proposals are for 10 — no Full for 20",
            input: vec![
                proposal_input(&c2, a2, 1, 10, 0),
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c1, a1, 0, 10, -1),
                value_input(a1, 0, 20, Validity::Valid),
            ],
            expect_full_for: vec![(0, 10), (1, 10)],
            expect_not_full_for: vec![(0, 20)],
            proposals_for: (proposed_value(a1, 0, 20, Validity::Valid), vec![]),
        },
        Case {
            name: "POL: value id 10 vs proposal id 20 — no Full for 20 at r0/r1 (partials only)",
            input: vec![
                proposal_input(&c1, a1, 0, 20, -1),
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c2, a2, 1, 20, 0),
            ],
            expect_full_for: vec![],
            expect_not_full_for: vec![(0, 10), (0, 20), (1, 20)],
            proposals_for: (proposed_value(a1, 0, 20, Validity::Valid), vec![]),
        },
        Case {
            name: "POL: values 10 and 20 at r0; pol proposals r1 for 10 and 20",
            input: vec![
                value_input(a1, 0, 10, Validity::Valid),
                proposal_input(&c1, a1, 0, 20, -1),
                value_input(a1, 0, 20, Validity::Valid),
                proposal_input(&c2, a2, 1, 10, 0),
                proposal_input(&c2, a2, 1, 20, 0),
            ],
            expect_full_for: vec![(0, 20), (1, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 20, Validity::Valid),
                vec![
                    signed_proposal(&c1, a1, 0, 20, -1),
                    signed_proposal(&c2, a2, 1, 20, 0),
                ],
            ),
        },
        Case {
            name: "POL: pending proposals r0/r1/r2 then value — upgrade_matching fills all",
            input: vec![
                proposal_input(&c1, a1, 1, 10, 0),
                proposal_input(&c2, a2, 0, 10, -1),
                proposal_input(&c1, a1, 2, 10, 0),
                value_input(a1, 0, 10, Validity::Valid),
            ],
            expect_full_for: vec![(0, 10), (1, 10), (2, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![
                    signed_proposal(&c2, a2, 0, 10, -1),
                    signed_proposal(&c1, a1, 1, 10, 0),
                    signed_proposal(&c1, a1, 2, 10, 0),
                ],
            ),
        },
        Case {
            name: "POL: same value at r0 and r2, then proposals r1/r3 — all Full",
            input: vec![
                value_input(a1, 0, 10, Validity::Valid),
                value_input(a1, 2, 10, Validity::Valid),
                proposal_input(&c1, a1, 1, 10, 0),
                proposal_input(&c2, a2, 3, 10, 2),
            ],
            expect_full_for: vec![(1, 10), (3, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![
                    signed_proposal(&c1, a1, 1, 10, 0),
                    signed_proposal(&c2, a2, 3, 10, 2),
                ],
            ),
        },
        Case {
            name: "POL: proposals at r1 and r3 then same value at r0 and r2",
            input: vec![
                proposal_input(&c1, a1, 1, 10, 0),
                proposal_input(&c2, a2, 3, 10, 2),
                value_input(a1, 0, 10, Validity::Valid),
                value_input(a1, 2, 10, Validity::Valid),
            ],
            expect_full_for: vec![(1, 10), (3, 10)],
            expect_not_full_for: vec![],
            proposals_for: (
                proposed_value(a1, 0, 10, Validity::Valid),
                vec![
                    signed_proposal(&c1, a1, 1, 10, 0),
                    signed_proposal(&c2, a2, 3, 10, 2),
                ],
            ),
        },
    ];

    for case in cases {
        println!("{}", case.name);
        let mut keeper = FullProposalKeeper::<TestContext>::new();

        for msg in case.input {
            match msg {
                Input::Proposal(p) => {
                    let _ = keeper.store_proposal(p, false);
                }
                Input::ProposedValue(v, _) => keeper.store_value(&v),
                _ => {}
            }
        }
        for (r, v) in &case.expect_full_for {
            assert!(
                full_proposal_at(&keeper, *r, *v).is_some(),
                "{}: expected Full for r{} v{}",
                case.name,
                r,
                v
            );
        }
        for (r, v) in &case.expect_not_full_for {
            assert!(
                full_proposal_at(&keeper, *r, *v).is_none(),
                "{}: expected not Full for r{} v{}",
                case.name,
                r,
                v
            );
        }
        assert_eq!(
            proposals_for_proposed_value(&keeper, &case.proposals_for.0),
            case.proposals_for.1,
            "{}",
            case.name
        );
    }
}

/// When a value is first received as Invalid and then upgraded to Valid,
/// a Full entry at a higher round (created via store_proposal's new_entry
/// matching the value by id across the height) must have its validity reconciled.
#[test]
fn validity_upgrade_propagates_to_higher_round_full_entry() {
    let [(v1, _), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let a2 = v2.address;
    let c2 = Ed25519Signer::new(sk2);

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // 1. Value arrives at round 0 as Invalid
    keeper.store_value(&proposed_value(a1, 0, 10, Validity::Invalid));

    // Stored value is Invalid, no full proposal yet
    let (_, validity) = keeper
        .get_value_by_id(&Height::new(1), &Value::new(10).id())
        .expect("value should exist");
    assert_eq!(validity, Validity::Invalid);
    assert!(full_proposal_at(&keeper, 0, 10).is_none());

    // 2. Proposal at round 1 with pol_round=0 arrives.
    let _ = keeper.store_proposal(signed_proposal(&c2, a2, 1, 10, 0), false);

    // Round 1 now has an Invalid full proposal (inherits the stored value's validity)
    let fp = full_proposal_at(&keeper, 1, 10).expect("full proposal should exist at round 1");
    assert_eq!(fp.validity, Validity::Invalid);

    // Round 0 still has ValueOnly — no proposal was stored at round 0, so no full proposal
    assert!(full_proposal_at(&keeper, 0, 10).is_none());
    let (_, validity) = keeper
        .get_value_by_id(&Height::new(1), &Value::new(10).id())
        .expect("value should still exist");
    assert_eq!(validity, Validity::Invalid);

    // 3. Value at round 0 is upgraded from Invalid to Valid
    keeper.store_value(&proposed_value(a1, 0, 10, Validity::Valid));

    // Stored value is now Valid
    let (_, validity) = keeper
        .get_value_by_id(&Height::new(1), &Value::new(10).id())
        .expect("get_value_by_id should find the value");
    assert_eq!(
        validity,
        Validity::Valid,
        "get_value_by_id: validity should be upgraded from Invalid to Valid"
    );

    // The full proposal at round 1 must now reflect Valid
    let fp = full_proposal_at(&keeper, 1, 10).unwrap();
    assert_eq!(
        fp.validity,
        Validity::Valid,
        "full_proposal_at_round_and_value: validity should be upgraded from Invalid to Valid"
    );
}

/// When a value is upgraded from Invalid to Valid, every Full entry at a higher
/// round referencing the same value id must have its validity reconciled, not
/// just one.
#[test]
fn validity_upgrade_propagates_to_all_higher_round_full_entries() {
    let [(v1, _), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let a2 = v2.address;
    let c2 = Ed25519Signer::new(sk2);

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // 1. Value at round 0 arrives as Invalid.
    keeper.store_value(&proposed_value(a1, 0, 10, Validity::Invalid));

    // 2. Two proposals at rounds 1 and 2, both with pol_round=0, create
    //    Full entries that inherit Invalid from the value at round 0.
    let _ = keeper.store_proposal(signed_proposal(&c2, a2, 1, 10, 0), false);
    let _ = keeper.store_proposal(signed_proposal(&c2, a2, 2, 10, 0), false);

    assert_eq!(
        full_proposal_at(&keeper, 1, 10)
            .expect("full proposal should exist at round 1")
            .validity,
        Validity::Invalid
    );
    assert_eq!(
        full_proposal_at(&keeper, 2, 10)
            .expect("full proposal should exist at round 2")
            .validity,
        Validity::Invalid
    );

    // 3. Upgrade the value at round 0 to Valid.
    keeper.store_value(&proposed_value(a1, 0, 10, Validity::Valid));

    // 4. Both Full entries must now reflect Valid — the reconciliation loop
    //    must visit every matching higher-round entry, not stop at the first.
    assert_eq!(
        full_proposal_at(&keeper, 1, 10)
            .expect("full proposal should still exist at round 1")
            .validity,
        Validity::Valid,
        "Full at round 1 should be upgraded to Valid"
    );
    assert_eq!(
        full_proposal_at(&keeper, 2, 10)
            .expect("full proposal should still exist at round 2")
            .validity,
        Validity::Valid,
        "Full at round 2 should be upgraded to Valid"
    );
}

/// `ValueOnly(v, validity)` and `Full(v, validity, proposal)` carry the same
/// `(value, validity)` payload — only proposal presence distinguishes them. The
/// `FullProposalKeeper` must therefore reconcile validity uniformly across both
/// kinds when divergent state exists for the same `(height, value_id)`.
///
/// This test installs two entries at different rounds with conflicting validity
/// (`Invalid` at round 1, `Valid` at round 2) and asserts the keeper converges to
/// `Valid` everywhere — under all four `(Kind, Kind)` configurations of the two
/// entries. If any one configuration diverges from the others, validity
/// reconciliation is treating `ValueOnly` and `Full` non-uniformly.
#[test]
fn validity_reconciles_uniformly_across_value_only_and_full() {
    let [(v1, _), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let a2 = v2.address;
    let c2 = Ed25519Signer::new(sk2);

    #[derive(Copy, Clone, Debug)]
    enum Kind {
        ValueOnly,
        Full,
    }

    let install =
        |k: &mut FullProposalKeeper<TestContext>, round: u32, validity: Validity, kind: Kind| {
            k.store_value(&proposed_value(a1, round, 10, validity));
            if matches!(kind, Kind::Full) {
                let _ = k.store_proposal(signed_proposal(&c2, a2, round, 10, -1), false);
            }
        };

    let configs = [
        (Kind::ValueOnly, Kind::ValueOnly),
        (Kind::ValueOnly, Kind::Full),
        (Kind::Full, Kind::ValueOnly),
        (Kind::Full, Kind::Full),
    ];

    for (k1, k2) in configs {
        let mut keeper = FullProposalKeeper::<TestContext>::new();

        // Round 1 (lower) gets Invalid; round 2 (higher) gets Valid.
        // The lower round is encountered first by `get_value_by_id`, so a
        // failure to reconcile the lower entry up to Valid is observable.
        install(&mut keeper, 1, Validity::Invalid, k1);
        install(&mut keeper, 2, Validity::Valid, k2);

        let (_, validity) = keeper
            .get_value_by_id(&Height::new(1), &Value::new(10).id())
            .unwrap_or_else(|| panic!("config ({:?}, {:?}): value should be stored", k1, k2));
        assert_eq!(
            validity,
            Validity::Valid,
            "config ({:?}, {:?}): get_value_by_id should converge to Valid",
            k1,
            k2
        );

        if matches!(k1, Kind::Full) {
            let fp = full_proposal_at(&keeper, 1, 10).unwrap_or_else(|| {
                panic!("config ({:?}, {:?}): Full should exist at round 1", k1, k2)
            });
            assert_eq!(
                fp.validity,
                Validity::Valid,
                "config ({:?}, {:?}): Full at round 1 should be Valid",
                k1,
                k2
            );
        }
        if matches!(k2, Kind::Full) {
            let fp = full_proposal_at(&keeper, 2, 10).unwrap_or_else(|| {
                panic!("config ({:?}, {:?}): Full should exist at round 2", k1, k2)
            });
            assert_eq!(
                fp.validity,
                Validity::Valid,
                "config ({:?}, {:?}): Full at round 2 should be Valid",
                k1,
                k2
            );
        }
    }
}

/// A `Valid -> Invalid` change from the application is rejected.
/// A stored `Valid` for a value id stays `Valid` even when a
/// later `store_value` for the same value at another round reports `Invalid`.
/// This matches `handle_validity_change`'s rule and surfaces the contradiction
/// via the `error!` log rather than silently smoothing it over.
#[test]
fn prior_valid_is_not_downgraded_by_later_invalid_at_other_round() {
    let [(v1, _), (v2, sk2)] = make_validators([1, 1]);
    let a1 = v1.address;
    let a2 = v2.address;
    let c2 = Ed25519Signer::new(sk2);

    #[derive(Copy, Clone, Debug)]
    enum Kind {
        ValueOnly,
        Full,
    }

    let install =
        |k: &mut FullProposalKeeper<TestContext>, round: u32, validity: Validity, kind: Kind| {
            k.store_value(&proposed_value(a1, round, 10, validity));
            if matches!(kind, Kind::Full) {
                let _ = k.store_proposal(signed_proposal(&c2, a2, round, 10, -1), false);
            }
        };

    for kind in [Kind::ValueOnly, Kind::Full] {
        let mut keeper = FullProposalKeeper::<TestContext>::new();

        // Prior `Valid` at round 1 in the form under test.
        install(&mut keeper, 1, Validity::Valid, kind);

        // Application now reports `Invalid` for the same value at round 2.
        keeper.store_value(&proposed_value(a1, 2, 10, Validity::Invalid));

        // The prior `Valid` at round 1 must NOT be downgraded.
        match kind {
            Kind::Full => {
                let fp = full_proposal_at(&keeper, 1, 10)
                    .unwrap_or_else(|| panic!("kind {:?}: Full should exist at round 1", kind));
                assert_eq!(
                    fp.validity,
                    Validity::Valid,
                    "kind {:?}: Full at round 1 must stay Valid (Valid -> Invalid is rejected)",
                    kind
                );
            }
            Kind::ValueOnly => {
                let (_, validity) = keeper
                    .get_value_by_id(&Height::new(1), &Value::new(10).id())
                    .unwrap_or_else(|| panic!("kind {:?}: value should be stored", kind));
                assert_eq!(
                    validity,
                    Validity::Valid,
                    "kind {:?}: get_value_by_id must stay Valid (Valid -> Invalid is rejected)",
                    kind
                );
            }
        }
    }
}

#[test]
fn store_proposal_surfaces_equivocation_against_proposal_only_entry() {
    let [(v, sk)] = make_validators([1]);
    let signer = Ed25519Signer::new(sk);
    let addr = v.address;

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Feed the first proposal with no matching value — it is kept as `Entry::ProposalOnly`.
    let first = signed_proposal(&signer, addr, 0, 10, -1);
    assert!(matches!(
        keeper.store_proposal(first.clone(), false),
        StoreProposalResult::Stored
    ));

    // An exact duplicate of the first proposal is silently ignored.
    assert!(matches!(
        keeper.store_proposal(first, false),
        StoreProposalResult::DuplicateIgnored
    ));

    // A second proposal from the same proposer for the same `(height, round, value)`
    // with a different `pol_round` is surfaced as equivocation against the still-
    // `Entry::ProposalOnly` first proposal.
    let second = signed_proposal(&signer, addr, 0, 10, 0);
    let expected_existing = signed_proposal(&signer, addr, 0, 10, -1);
    let result = keeper.store_proposal(second.clone(), false);
    let (existing, conflicting) = match result {
        StoreProposalResult::Equivocation {
            existing,
            conflicting,
        } => (existing, conflicting),
        other => panic!("expected equivocation, got {other:?}"),
    };
    assert_eq!(existing, expected_existing);
    assert_eq!(conflicting, second);
}

#[test]
fn would_append_distinct_reports_when_the_cap_is_reached() {
    let [(v, sk)] = make_validators([1]);
    let signer = Ed25519Signer::new(sk);
    let addr = v.address;

    let mut keeper = FullProposalKeeper::<TestContext>::new();
    let height = Height::new(1);
    let round = Round::new(0);

    // Empty bucket: the first entry is always allowed.
    assert!(!keeper.would_append_distinct(height, round, &Value::new(10).id()));

    let _ = keeper.store_proposal(signed_proposal(&signer, addr, 0, 10, -1), false);
    // One entry, still under the cap.
    assert!(!keeper.would_append_distinct(height, round, &Value::new(20).id()));

    let _ = keeper.store_proposal(signed_proposal(&signer, addr, 0, 20, -1), false);
    // At the cap: a new distinct value id would append, an already-present one would not.
    assert!(keeper.would_append_distinct(height, round, &Value::new(30).id()));
    assert!(!keeper.would_append_distinct(height, round, &Value::new(10).id()));
    // Other rounds are independent.
    assert!(!keeper.would_append_distinct(height, Round::new(1), &Value::new(30).id()));
}

#[test]
fn store_proposal_caps_distinct_entries_per_round() {
    let [(v, sk)] = make_validators([1]);
    let signer = Ed25519Signer::new(sk);
    let addr = v.address;

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Two distinct proposals from the same proposer at the same round fill the bucket.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 10, -1), false),
        StoreProposalResult::Stored
    ));
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 20, -1), false),
        StoreProposalResult::Stored
    ));

    // A third distinct proposal is rejected at the cap.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 30, -1), false),
        StoreProposalResult::CapReached
    ));

    // The two stored proposals still pair with their values; the rejected one never forms a `Full`.
    keeper.store_value(&proposed_value(addr, 0, 10, Validity::Valid));
    keeper.store_value(&proposed_value(addr, 0, 20, Validity::Valid));
    assert!(full_proposal_at(&keeper, 0, 10).is_some());
    assert!(full_proposal_at(&keeper, 0, 20).is_some());
    assert!(full_proposal_at(&keeper, 0, 30).is_none());
}

#[test]
fn mixed_entry_bucket_caps_third_distinct_proposal() {
    let [(v, sk)] = make_validators([1]);
    let signer = Ed25519Signer::new(sk);
    let addr = v.address;

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Fill the bucket with one `ProposalOnly` and one `ValueOnly` entry (two distinct value ids).
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 10, -1), false),
        StoreProposalResult::Stored
    ));
    keeper.store_value(&proposed_value(addr, 0, 20, Validity::Valid));

    // A third distinct proposal is rejected at the cap.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 30, -1), false),
        StoreProposalResult::CapReached
    ));

    // A proposal matching the existing `ValueOnly(20)` upgrades it to `Full` for free — it does
    // not append, so the cap does not reject it.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 20, -1), false),
        StoreProposalResult::Stored
    ));
    assert!(full_proposal_at(&keeper, 0, 20).is_some());
    assert!(full_proposal_at(&keeper, 0, 30).is_none());
}

#[test]
fn cap_exempt_proposal_is_stored_beyond_the_cap() {
    let [(v, sk)] = make_validators([1]);
    let signer = Ed25519Signer::new(sk);
    let addr = v.address;

    let mut keeper = FullProposalKeeper::<TestContext>::new();

    // Two distinct proposals fill the bucket.
    let _ = keeper.store_proposal(signed_proposal(&signer, addr, 0, 10, -1), false);
    let _ = keeper.store_proposal(signed_proposal(&signer, addr, 0, 20, -1), false);

    // A third distinct proposal is stored when the caller declares it exempt.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 30, -1), true),
        StoreProposalResult::Stored
    ));
    keeper.store_value(&proposed_value(addr, 0, 30, Validity::Valid));
    assert!(full_proposal_at(&keeper, 0, 30).is_some());

    // The cap still applies to entries that are not exempt.
    assert!(matches!(
        keeper.store_proposal(signed_proposal(&signer, addr, 0, 40, -1), false),
        StoreProposalResult::CapReached
    ));
}
