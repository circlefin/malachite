use malachitebft_core_types::{Round, SignedProposal};
use malachitebft_test::{Address, Height, PrivateKey, Proposal, TestContext, Value};

use arc_malachitebft_core_driver::proposal_keeper::EvidenceMap;

fn pk(id: &str) -> PrivateKey {
    let mut seed = [0u8; 32];
    for (i, b) in id.bytes().enumerate() {
        seed[i % 32] = b;
    }
    PrivateKey::from(seed)
}

fn addr(id: &str) -> Address {
    Address::from_public_key(&pk(id).public_key())
}

fn make_proposal_pair(
    addr_id: &str,
    round: u32,
    values: [u64; 2],
) -> (SignedProposal<TestContext>, SignedProposal<TestContext>) {
    let pk = pk(addr_id);
    let addr = addr(addr_id);
    let round = Round::new(round);

    let p1 = Proposal::new(
        Height::new(1),
        round,
        Value::new(values[0]),
        Round::Nil,
        addr,
    );
    let p2 = Proposal::new(
        Height::new(1),
        round,
        Value::new(values[1]),
        Round::Nil,
        addr,
    );

    (
        SignedProposal::new(p1.clone(), pk.sign(&p1.to_sign_bytes())),
        SignedProposal::new(p2.clone(), pk.sign(&p2.to_sign_bytes())),
    )
}

struct TestCase {
    name: &'static str,
    evidence: &'static [(&'static str, u32, [u64; 2])], // (addr, round, [v1, v2])
    expected: &'static [(&'static str, usize)],         // (addr, count)
}

#[test]
fn test_proposal_evidence_deduplication() {
    let cases: &[TestCase] = &[
        TestCase {
            name: "single proposal equivocation",
            evidence: &[("Alice", 0, [100, 200])],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "duplicate same order",
            evidence: &[("Alice", 0, [100, 200]), ("Alice", 0, [100, 200])],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "duplicate reversed order",
            evidence: &[("Alice", 0, [100, 200]), ("Alice", 0, [200, 100])],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "different rounds not deduped",
            evidence: &[("Alice", 0, [100, 200]), ("Alice", 1, [100, 200])],
            expected: &[("Alice", 2)],
        },
        TestCase {
            name: "multiple validators",
            evidence: &[
                ("Alice", 0, [100, 200]),
                ("Bob", 0, [100, 200]),
                ("Alice", 0, [100, 200]), // duplicate
            ],
            expected: &[("Alice", 1), ("Bob", 1)],
        },
    ];

    for case in cases {
        let mut evidence = EvidenceMap::<TestContext>::new();

        for &(addr_id, round, values) in case.evidence {
            let (p1, p2) = make_proposal_pair(addr_id, round, values);
            evidence.add(p1, p2);
        }

        for &(addr_id, expected_count) in case.expected {
            let actual = evidence.get(&addr(addr_id)).map(|v| v.len()).unwrap_or(0);
            assert_eq!(
                actual, expected_count,
                "Test '{}' failed for {}: expected {}, got {}",
                case.name, addr_id, expected_count, actual
            );
        }
    }
}
