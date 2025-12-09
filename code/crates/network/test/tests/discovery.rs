use std::{time::Duration, vec};

use informalsystems_malachitebft_discovery_test::{Expected, Test, TestNode};
use malachitebft_network::{BootstrapProtocol, DiscoveryConfig, Selector};

// Ensuring that having the node's address in the bootstrap set does not cause
// any issues.
#[tokio::test]
pub async fn bootstrap_set_with_self() {
    let test = Test::new(
        [
            TestNode::correct(0, vec![0, 1]),
            TestNode::correct(1, vec![0, 1]),
        ],
        [Expected::Exactly(vec![1]), Expected::Exactly(vec![0])],
        Duration::from_secs(0),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: true,
            bootstrap_protocol: BootstrapProtocol::Full,
            selector: Selector::Random,
            ..Default::default()
        },
    );

    test.run().await
}

// Testing the following circular bootstrap sets graph:
//     0 <--- 1 <--- 2 <--- 3 <--- 4
//     |                           ^
//     +---------------------------+
#[tokio::test]
pub async fn circular_graph() {
    let test = Test::new(
        [
            TestNode::correct(0, vec![4]),
            TestNode::correct(1, vec![0]),
            TestNode::correct(2, vec![1]),
            TestNode::correct(3, vec![2]),
            TestNode::correct(4, vec![3]),
        ],
        [
            Expected::Exactly(vec![1, 2, 3, 4]),
            Expected::Exactly(vec![0, 2, 3, 4]),
            Expected::Exactly(vec![0, 1, 3, 4]),
            Expected::Exactly(vec![0, 1, 2, 4]),
            Expected::Exactly(vec![0, 1, 2, 3]),
        ],
        Duration::from_secs(0),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: true,
            bootstrap_protocol: BootstrapProtocol::Full,
            selector: Selector::Random,
            ..Default::default()
        },
    );

    test.run().await
}

// Testing a circular bootstrap sets graph with N nodes.
#[tokio::test]
pub async fn circular_graph_n() {
    const N: usize = 10;

    let mut nodes = Vec::with_capacity(N);
    let mut expected = Vec::with_capacity(N);
    for i in 0..N {
        let bootstrap = if i == 0 { vec![N - 1] } else { vec![i - 1] };
        nodes.push(TestNode::correct(i, bootstrap));
        expected.push(Expected::Exactly(
            (0..N).filter(|&j| j != i).collect::<Vec<_>>(),
        ));
    }

    let test: Test<N> = Test::new(
        nodes.try_into().expect("Expected a Vec of length {N}"),
        expected.try_into().expect("Expected a Vec of length {N}"),
        Duration::from_secs(0),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: true,
            bootstrap_protocol: BootstrapProtocol::Full,
            selector: Selector::Random,
            ..Default::default()
        },
    );

    test.run().await
}

// Testing correctness when discovery is disabled. Especially, the nodes should
// not accept more connections than the defined number of inbound peers in the
// configuration.
// With discovery disabled all peers are considered inbound.
#[tokio::test]
pub async fn discovery_disabled() {
    let test = Test::new(
        [
            TestNode::correct(0, vec![]),
            TestNode::correct(1, vec![0]),
            TestNode::correct(2, vec![0, 1]),
            TestNode::correct(3, vec![1, 2]),
            TestNode::correct(4, vec![2, 3]),
            TestNode::correct(5, vec![3, 4]),
            TestNode::correct(6, vec![4, 5]),
            TestNode::correct(7, vec![5, 6]),
            TestNode::correct(8, vec![6, 7]),
            TestNode::correct(9, vec![7, 8]),
            TestNode::correct(10, vec![8, 9]),
        ],
        [
            Expected::Exactly(vec![1, 2]),        // node 0
            Expected::Exactly(vec![0, 2, 3]),     // node 1
            Expected::Exactly(vec![0, 1, 3, 4]),  // node 2
            Expected::Exactly(vec![1, 2, 4, 5]),  // node 3
            Expected::Exactly(vec![2, 3, 5, 6]),  // node 4
            Expected::Exactly(vec![3, 4, 6, 7]),  // node 5
            Expected::Exactly(vec![4, 5, 7, 8]),  // node 6
            Expected::Exactly(vec![5, 6, 8, 9]),  // node 7
            Expected::Exactly(vec![6, 7, 9, 10]), // node 8
            Expected::Exactly(vec![7, 8, 10]),    // node 9
            Expected::Exactly(vec![8, 9]),        // node 10
        ],
        Duration::from_secs(1),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: false,
            num_inbound_peers: 4,
            ..Default::default()
        },
    );

    test.run().await
}

// Ensuring that the discovery protocol can handle concurrent dials between nodes.
#[tokio::test]
pub async fn discovery_concurrent_dial() {
    const N: usize = 10;

    let mut nodes = Vec::with_capacity(N);
    let mut expected = Vec::with_capacity(N);
    for i in 0..N {
        let bootstrap = (0..N).filter(|&j| j != i).collect::<Vec<_>>();
        nodes.push(TestNode::correct(i, bootstrap));
        expected.push(Expected::Exactly(
            (0..N).filter(|&j| j != i).collect::<Vec<_>>(),
        ));
    }

    let test: Test<N> = Test::new(
        nodes.try_into().expect("Expected a Vec of length {N}"),
        expected.try_into().expect("Expected a Vec of length {N}"),
        Duration::from_secs(0),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: true,
            bootstrap_protocol: BootstrapProtocol::Full,
            selector: Selector::Random,
            ephemeral_connection_timeout: Duration::from_secs(3),
            ..Default::default()
        },
    );

    test.run().await
}

// Test multiple nodes with persistent_peers_only enabled
#[tokio::test]
pub async fn multiple_persistent_peers_only_nodes() {
    let test = Test::new(
        [
            // Node 0: persistent_peers_only=true, allows 1 and 2
            TestNode::with_custom_config(0, vec![1, 2], |config| {
                config.discovery.persistent_peers_only = true;
            }),
            // Node 1: persistent_peers_only=true, allows 0 and 3
            TestNode::with_custom_config(1, vec![0, 3], |config| {
                config.discovery.persistent_peers_only = true;
            }),
            // Node 2: normal node, tries to connect to 0 and 3
            TestNode::correct(2, vec![0, 3]),
            // Node 3: normal node, tries to connect to 1 and 2
            TestNode::correct(3, vec![1, 2]),
        ],
        [
            Expected::AtLeast(vec![1, 2]), // Node 0: connects to at least 1,2 (both persistent)
            Expected::AtLeast(vec![0, 3]), // Node 1: connects to at least 0,3 (both persistent)
            Expected::AtLeast(vec![0, 3]), // Node 2: connects to at least 0 (in 0's list) and 3
            Expected::AtLeast(vec![1, 2]), // Node 3: connects to at least 1 (in 1's list) and 2
        ],
        Duration::from_secs(2),
        Duration::from_secs(20),
        DiscoveryConfig {
            enabled: false, // Disabled for explicit control
            num_inbound_peers: 10,
            ..Default::default()
        },
    );

    test.run().await
}

// Test persistent_peers_only with discovery enabled
// Node should discover peers but only connect to persistent ones
#[tokio::test]
pub async fn persistent_peers_only_with_discovery_enabled() {
    let test = Test::new(
        [
            // Node 0: persistent_peers_only=true, bootstrap=[1], will discover 2,3 but reject them
            TestNode::with_custom_config(0, vec![1], |config| {
                config.discovery.persistent_peers_only = true;
            }),
            // Node 1: connects to 0, 2, 3
            TestNode::correct(1, vec![0, 2, 3]),
            // Node 2: connects via node 1
            TestNode::correct(2, vec![1]),
            // Node 3: connects via node 1
            TestNode::correct(3, vec![1]),
        ],
        [
            Expected::Exactly(vec![1]),       // Node 0 only connects to 1 (persistent)
            Expected::AtLeast(vec![0, 2, 3]), // Node 1 connects to all (may discover more)
            Expected::AtLeast(vec![1]),       // Node 2 connects to at least 1 (may discover 3)
            Expected::AtLeast(vec![1]),       // Node 3 connects to at least 1 (may discover 2)
        ],
        Duration::from_secs(1),
        Duration::from_secs(20),
        DiscoveryConfig {
            enabled: true,
            bootstrap_protocol: BootstrapProtocol::Full,
            selector: Selector::Random,
            num_inbound_peers: 10,
            num_outbound_peers: 10,
            ..Default::default()
        },
    );

    test.run().await
}
