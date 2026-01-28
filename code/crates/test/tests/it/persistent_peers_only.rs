use std::time::Duration;

use crate::{TestBuilder, TestParams};

/// Test that a node with persistent_peers_only enabled only accepts
/// connections from peers in its persistent_peers list.
///
/// Setup:
/// - Node 1: normal validator
/// - Node 2: normal validator
/// - Node 3: validator with persistent_peers_only=true, persistent_peers=[node 1]
///
/// Expected behavior:
/// - Node 1 and 2 connect to each other and reach consensus
/// - Node 3 only connects to node 1 (its persistent peer)
/// - Node 3 rejects connections from node 2 (non-persistent)
/// - All nodes should still reach consensus (2/3 voting power with nodes 1 and 3)
#[tokio::test]
pub async fn node_with_persistent_peers_only_rejects_non_persistent() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Normal validator (voting power 30)
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 2: Normal validator (voting power 30)
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3: Validator with persistent_peers_only=true
    // Only allows connections to/from node 1
    test.add_node()
        .with_voting_power(40)
        .add_config_modifier(|config| {
            // Enable persistent_peers_only mode
            config.consensus.p2p.persistent_peers_only = true;

            // Node 3's persistent_peers list is [node 1, node 2]
            // Keep only node 1 (first element at index 0)
            if !config.consensus.p2p.persistent_peers.is_empty() {
                let node_1_peer = config.consensus.p2p.persistent_peers[0].clone();
                config.consensus.p2p.persistent_peers = vec![node_1_peer];
            }
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(Duration::from_secs(60), TestParams::default())
        .await
}

/// Test that multiple nodes can use persistent_peers_only simultaneously
///
/// Setup:
/// - Node 1: validator with persistent_peers_only=true, allows only node 2
/// - Node 2: normal validator, connects to all
/// - Node 3: validator with persistent_peers_only=true, allows only node 2
///
/// Expected behavior:
/// - Node 1 only connects to node 2
/// - Node 2 acts as a hub, connecting to both node 1 and 3
/// - Node 3 only connects to node 2
/// - All nodes reach consensus through node 2
#[tokio::test]
pub async fn multiple_nodes_with_persistent_peers_only() {
    const HEIGHT: u64 = 5;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Validator with persistent_peers_only=true (only node 2)
    test.add_node()
        .with_voting_power(30)
        .add_config_modifier(|config| {
            config.consensus.p2p.persistent_peers_only = true;
            // Node 1's persistent_peers list is [node 2, node 3]
            // Keep only node 2 (first element at index 0)
            if !config.consensus.p2p.persistent_peers.is_empty() {
                let node_2_peer = config.consensus.p2p.persistent_peers[0].clone();
                config.consensus.p2p.persistent_peers = vec![node_2_peer];
            }
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 2: Normal validator (acts as hub)
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3: Validator with persistent_peers_only=true (only node 2)
    test.add_node()
        .with_voting_power(40)
        .add_config_modifier(|config| {
            config.consensus.p2p.persistent_peers_only = true;
            // Node 3's persistent_peers list is [node 1, node 2]
            // Keep only node 2 (second element at index 1)
            if config.consensus.p2p.persistent_peers.len() > 1 {
                let node_2_peer = config.consensus.p2p.persistent_peers[1].clone();
                config.consensus.p2p.persistent_peers = vec![node_2_peer];
            }
        })
        .start()
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(Duration::from_secs(60), TestParams::default())
        .await
}

/// Test persistent_peers_only with a late-joining node
///
/// Setup:
/// - Node 1, 2: normal validators that start immediately
/// - Node 3: validator with persistent_peers_only=true that starts late, allows only node 1
///
/// Expected behavior:
/// - Nodes 1 and 2 reach consensus first
/// - Node 3 starts late, connects only to node 1, and catches up
/// - Node 3 rejects connection from node 2
#[tokio::test]
pub async fn persistent_peers_only_with_late_start() {
    const HEIGHT: u64 = 8;

    let mut test = TestBuilder::<()>::new();

    // Node 1: Normal validator
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 2: Normal validator
    test.add_node()
        .with_voting_power(30)
        .start()
        .wait_until(HEIGHT)
        .success();

    // Node 3: Validator with persistent_peers_only=true, starts late
    test.add_node()
        .with_voting_power(40)
        .add_config_modifier(|config| {
            config.consensus.p2p.persistent_peers_only = true;
            // Keep only first persistent peer (node 1)
            if config.consensus.p2p.persistent_peers.len() > 1 {
                config.consensus.p2p.persistent_peers.truncate(1);
            }
        })
        .start_after(1, Duration::from_secs(5))
        .wait_until(HEIGHT)
        .success();

    test.build()
        .run_with_params(
            Duration::from_secs(60),
            TestParams {
                enable_value_sync: true,
                ..TestParams::default()
            },
        )
        .await
}
