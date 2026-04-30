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

use arc_malachitebft_core_consensus::full_proposal::{FullProposal, FullProposalKeeper};
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
                Input::Proposal(p) => keeper.store_proposal(p),
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
