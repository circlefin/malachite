use informalsystems_malachitebft_test::{Height, TestContext};
use malachitebft_sync::{PeerId, State, Status};
use std::collections::BTreeMap;

#[test]
fn filter_peers_by_range_test() {
    let peer1 = PeerId::random();
    let peer2 = PeerId::random();

    struct TestCase {
        name: &'static str,
        peers: Vec<(PeerId, u64, u64)>,
        range: std::ops::RangeInclusive<Height>,
        exclude_peer: Option<PeerId>,
        expected_peers: Vec<PeerId>,
        expected_ranges: Vec<(u64, u64)>, // (start, end) for each expected peer
    }

    let test_cases = vec![
        TestCase {
            name: "no peers",
            peers: vec![],
            range: Height::new(1)..=Height::new(20),
            exclude_peer: None,
            expected_peers: vec![],
            expected_ranges: vec![],
        },
        TestCase {
            name: "peers providing the full range, no exclusion",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(13)..=Height::new(15),
            exclude_peer: None,
            expected_peers: vec![peer1, peer2],
            expected_ranges: vec![(13, 15), (13, 15)],
        },
        TestCase {
            name: "peers providing the full range, excluding one peer",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(13)..=Height::new(15),
            exclude_peer: Some(peer1),
            expected_peers: vec![peer2],
            expected_ranges: vec![(13, 15)],
        },
        TestCase {
            name: "one peer providing a prefix range, no exclusion",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(17)..=Height::new(30),
            exclude_peer: None,
            expected_peers: vec![peer2],
            expected_ranges: vec![(17, 20)],
        },
        TestCase {
            name: "one peer providing a prefix range, excluding one peer",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(17)..=Height::new(30),
            exclude_peer: Some(peer2),
            expected_peers: vec![],
            expected_ranges: vec![],
        },
        TestCase {
            name: "no peers providing start height, no exclusion",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(5)..=Height::new(10),
            exclude_peer: None,
            expected_peers: vec![],
            expected_ranges: vec![],
        },
        TestCase {
            name: "no peers providing the range, no exclusion",
            peers: vec![(peer1, 10, 15), (peer2, 10, 20)],
            range: Height::new(21)..=Height::new(30),
            exclude_peer: None,
            expected_peers: vec![],
            expected_ranges: vec![],
        },
    ];

    for test_case in test_cases {
        // Setup peers for this test case
        let mut peers = BTreeMap::new();
        for (peer_id, min, max) in &test_case.peers {
            peers.insert(
                *peer_id,
                Status::<TestContext> {
                    peer_id: *peer_id,
                    tip_height: Height::new(*max),
                    history_min_height: Height::new(*min),
                },
            );
        }

        let filtered_peers = State::<TestContext>::filter_peers_by_range(
            &peers,
            &test_case.range,
            test_case.exclude_peer,
        );

        // Validate expected number of peers
        assert_eq!(
            filtered_peers.len(),
            test_case.expected_peers.len(),
            "Test case '{}': expected {} peers, got {}",
            test_case.name,
            test_case.expected_peers.len(),
            filtered_peers.len()
        );

        // Validate each expected peer is present with correct range
        for (i, expected_peer) in test_case.expected_peers.iter().enumerate() {
            assert!(
                filtered_peers.contains_key(expected_peer),
                "Test case '{}': expected peer {:?} not found",
                test_case.name,
                expected_peer
            );

            let peer_range = filtered_peers.get(expected_peer).unwrap();
            let (expected_start, expected_end) = test_case.expected_ranges[i];

            assert_eq!(
                peer_range.start().as_u64(),
                expected_start,
                "Test case '{}': peer {:?} has wrong start, expected {}, got {}",
                test_case.name,
                expected_peer,
                expected_start,
                peer_range.start().as_u64()
            );

            assert_eq!(
                peer_range.end().as_u64(),
                expected_end,
                "Test case '{}': peer {:?} has wrong end, expected {}, got {}",
                test_case.name,
                expected_peer,
                expected_end,
                peer_range.end().as_u64()
            );
        }
    }
}
