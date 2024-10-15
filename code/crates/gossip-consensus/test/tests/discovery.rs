#![allow(unused_crate_dependencies)]

use std::{time::Duration, vec};

use malachite_discovery_test::{Expected, Test, TestNode};

// Testing the following circular bootstrap sets graph:
//     0 ---> 1 ---> 2 ---> 3 ---> 4
//     ^                           |
//     +---------------------------+
#[tokio::test]
pub async fn circular_graph() {
    let test = Test::new(
        [
            TestNode::correct(0, vec![1]),
            TestNode::correct(1, vec![2]),
            TestNode::correct(2, vec![3]),
            TestNode::correct(3, vec![4]),
            TestNode::correct(4, vec![0]),
        ],
        [
            Expected::Exactly(vec![1, 2, 3, 4]),
            Expected::Exactly(vec![0, 2, 3, 4]),
            Expected::Exactly(vec![0, 1, 3, 4]),
            Expected::Exactly(vec![0, 1, 2, 4]),
            Expected::Exactly(vec![0, 1, 2, 3]),
        ],
        Duration::from_secs(0),
        Duration::from_secs(5),
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
        let bootstrap = vec![(i + 1) % N];
        nodes.push(TestNode::correct(i, bootstrap));
        expected.push(Expected::Exactly(
            (0..N).filter(|&j| j != i).collect::<Vec<_>>(),
        ));
    }

    let test: Test<N> = Test::new(
        nodes.try_into().expect("Expected a Vec of length 100"),
        expected.try_into().expect("Expected a Vec of length 100"),
        Duration::from_secs(0),
        Duration::from_secs(10),
    );

    test.run().await
}

// Testing the following weakly connected bootstrap sets graph:
//     0 <--> 1 <--- 2 ---> 3 <--> 4
#[tokio::test]
pub async fn weakly_connected_graph() {
    let test = Test::new(
        [
            TestNode::correct(0, vec![1]),
            TestNode::correct(1, vec![0]),
            TestNode::correct(2, vec![1, 3]),
            TestNode::correct(3, vec![4]),
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
        Duration::from_secs(5),
    );

    test.run().await
}
