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
            Expected::Exactly(vec![1, 2]),
            Expected::Exactly(vec![0, 2]),
            Expected::Exactly(vec![0, 1]),
            Expected::Exactly(vec![4, 5]),
            Expected::Exactly(vec![3, 5]),
            Expected::Exactly(vec![3, 4]),
            Expected::Exactly(vec![7, 8]),
            Expected::Exactly(vec![6, 8]),
            Expected::Exactly(vec![6, 7]),
            Expected::Exactly(vec![10]),
            Expected::Exactly(vec![9]),
        ],
        Duration::from_secs(1),
        Duration::from_secs(10),
        DiscoveryConfig {
            enabled: false,
            num_inbound_peers: 2,
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
