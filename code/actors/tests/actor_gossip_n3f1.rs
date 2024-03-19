#[path = "util.rs"]
mod util;
use util::*;

#[ignore]
#[tokio::test]
pub async fn discard_gossip_event_fail() {
    let nodes = Test::new(
        [
            TestNode::faulty(10, vec![Fault::DiscardGossipEvent(1.0)]),
            TestNode::faulty(5, vec![]),
            TestNode::faulty(5, vec![]),
        ],
        0,
    );

    run_test(nodes).await
}

#[ignore]
#[tokio::test]
pub async fn discard_gossip_event_ok() {
    let nodes = Test::new(
        [
            TestNode::faulty(5, vec![Fault::DiscardGossipEvent(1.0)]),
            TestNode::faulty(15, vec![]),
            TestNode::faulty(15, vec![]),
        ],
        6,
    );

    run_test(nodes).await
}
