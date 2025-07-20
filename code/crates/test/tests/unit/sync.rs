use informalsystems_malachitebft_test::{Height, TestContext};
use malachitebft_sync::{PeerId, State, Status};
use std::collections::BTreeMap;
use std::str::FromStr;

// Helper function to create a set of two peers
fn make_peers() -> (BTreeMap<PeerId, Status<TestContext>>, PeerId, PeerId) {
    let peer1 = PeerId::from_str("12D3KooWEPvZRT1FQXVpgXVsUsi76VV8ahjG7bZSeMY3JvkbDYk1").unwrap();
    let peer2 = PeerId::from_str("12D3KooWEPvZRT1FQXVpgXVsUsi76VV8ahjG7bZSeMY3JvkbDYk2").unwrap();
    let peers = BTreeMap::from([
        (
            peer1,
            Status::<TestContext> {
                peer_id: peer1,
                tip_height: Height::new(15),
                history_min_height: Height::new(10),
            },
        ),
        (
            peer2,
            Status::<TestContext> {
                peer_id: peer2,
                tip_height: Height::new(20),
                history_min_height: Height::new(10),
            },
        ),
    ]);
    (peers, peer1, peer2)
}

#[test]
fn filter_peers_empty_set_test() {
    // An empty set of peers
    let peers = BTreeMap::new();

    // Filter over an empty set of peers
    let range = Height::new(1)..=Height::new(20);
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, None);
    assert!(filtered_peers.is_empty());
}

#[test]
fn filter_peers_providing_full_range_test() {
    // Given a set of two peers
    let (peers, peer1, peer2) = make_peers();

    // Given a range of heights that can be provided by both peers
    let range = Height::new(13)..=Height::new(15);

    // Filter by the range and exclude none
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, None);
    assert_eq!(filtered_peers.len(), 2);
    assert!(filtered_peers.contains_key(&peer1));
    assert!(filtered_peers.contains_key(&peer2));
    assert_eq!(filtered_peers.get(&peer1).unwrap().start().as_u64(), 13);
    assert_eq!(filtered_peers.get(&peer1).unwrap().end().as_u64(), 15);
    assert_eq!(filtered_peers.get(&peer2).unwrap().start().as_u64(), 13);
    assert_eq!(filtered_peers.get(&peer2).unwrap().end().as_u64(), 15);

    // Filter by the range and exclude one peer
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, Some(peer1));
    assert_eq!(filtered_peers.len(), 1);
    assert!(!filtered_peers.contains_key(&peer1));
    assert!(filtered_peers.contains_key(&peer2));
    assert_eq!(filtered_peers.get(&peer2).unwrap().start().as_u64(), 13);
    assert_eq!(filtered_peers.get(&peer2).unwrap().end().as_u64(), 15);
}

#[test]
fn filter_peers_providing_prefix_range_test() {
    // Given a set of two peers
    let (peers, peer1, peer2) = make_peers();

    // Given a range of heights that can be partially provided by only one of the peers
    let range = Height::new(17)..=Height::new(30);

    // Filter by the range and exclude none
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, None);
    assert_eq!(filtered_peers.len(), 1);
    assert!(!filtered_peers.contains_key(&peer1));
    assert!(filtered_peers.contains_key(&peer2));
    assert_eq!(filtered_peers.get(&peer2).unwrap().start().as_u64(), 17);
    assert_eq!(filtered_peers.get(&peer2).unwrap().end().as_u64(), 20);

    // Filter by the range and exclude one peer
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, Some(peer2));
    assert!(filtered_peers.is_empty());
}

#[test]
fn filter_peers_not_providing_start_height_test() {
    // Given a set of two peers
    let (peers, _, _) = make_peers();

    // Given a range of heights with a start height not provided by any peer
    let range = Height::new(5)..=Height::new(20);

    // Filter by the range and exclude none
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, None);
    assert!(filtered_peers.is_empty());
}

#[test]
fn filter_peers_not_providing_range_test() {
    // Given a set of two peers
    let (peers, _, _) = make_peers();

    // Given a range of heights not provided by any peer
    let range = Height::new(21)..=Height::new(30);

    // Filter by the range and exclude none
    let filtered_peers = State::<TestContext>::filter_peers_by_range(&peers, &range, None);
    assert!(filtered_peers.is_empty());
}
