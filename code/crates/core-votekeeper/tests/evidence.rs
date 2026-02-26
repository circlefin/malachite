use malachitebft_core_types::{NilOrVal, Round, SignedVote};
use malachitebft_test::{Address, Height, PrivateKey, TestContext, ValueId, Vote};

use arc_malachitebft_core_votekeeper::EvidenceMap;

#[derive(Clone, Copy)]
enum VoteType {
    Prevote,
    Precommit,
}

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

fn make_vote_pair(
    addr_id: &str,
    vote_type: VoteType,
    round: u32,
    values: [u64; 2],
) -> (SignedVote<TestContext>, SignedVote<TestContext>) {
    let pk = pk(addr_id);
    let addr = addr(addr_id);
    let round = Round::new(round);

    let (v1, v2) = match vote_type {
        VoteType::Prevote => (
            Vote::new_prevote(
                Height::new(1),
                round,
                NilOrVal::Val(ValueId::new(values[0])),
                addr,
            ),
            Vote::new_prevote(
                Height::new(1),
                round,
                NilOrVal::Val(ValueId::new(values[1])),
                addr,
            ),
        ),
        VoteType::Precommit => (
            Vote::new_precommit(
                Height::new(1),
                round,
                NilOrVal::Val(ValueId::new(values[0])),
                addr,
            ),
            Vote::new_precommit(
                Height::new(1),
                round,
                NilOrVal::Val(ValueId::new(values[1])),
                addr,
            ),
        ),
    };

    (
        SignedVote::new(v1.clone(), pk.sign(&v1.to_sign_bytes())),
        SignedVote::new(v2.clone(), pk.sign(&v2.to_sign_bytes())),
    )
}

struct TestCase {
    name: &'static str,
    evidence: &'static [(&'static str, VoteType, u32, [u64; 2])], // (addr, type, round, [v1, v2])
    expected: &'static [(&'static str, usize)],                   // (addr, count)
}

use VoteType::*;

#[test]
fn test_vote_evidence_deduplication() {
    let cases: &[TestCase] = &[
        TestCase {
            name: "single prevote equivocation",
            evidence: &[("Alice", Prevote, 0, [100, 200])],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "duplicate same order",
            evidence: &[
                ("Alice", Prevote, 0, [100, 200]),
                ("Alice", Prevote, 0, [100, 200]),
            ],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "duplicate reversed order",
            evidence: &[
                ("Alice", Prevote, 0, [100, 200]),
                ("Alice", Prevote, 0, [200, 100]),
            ],
            expected: &[("Alice", 1)],
        },
        TestCase {
            name: "different rounds not deduped",
            evidence: &[
                ("Alice", Prevote, 0, [100, 200]),
                ("Alice", Prevote, 1, [100, 200]),
            ],
            expected: &[("Alice", 2)],
        },
        TestCase {
            name: "prevote and precommit not deduped",
            evidence: &[
                ("Alice", Prevote, 0, [100, 200]),
                ("Alice", Precommit, 0, [100, 200]),
            ],
            expected: &[("Alice", 2)],
        },
        TestCase {
            name: "multiple validators",
            evidence: &[
                ("Alice", Prevote, 0, [100, 200]),
                ("Bob", Prevote, 0, [100, 200]),
                ("Alice", Prevote, 0, [100, 200]), // duplicate
            ],
            expected: &[("Alice", 1), ("Bob", 1)],
        },
    ];

    for case in cases {
        let mut evidence = EvidenceMap::<TestContext>::new();

        for &(addr_id, vote_type, round, values) in case.evidence {
            let (v1, v2) = make_vote_pair(addr_id, vote_type, round, values);
            evidence.add(v1, v2);
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
