#[path = "util.rs"]
mod util;
use util::*;

#[tokio::test]
pub async fn discard_gossip_event_fail() {
    let nodes = Test::new(
        [
            TestNode::faulty(10, vec![Fault::DiscardGossipEvent(0.5)]),
            TestNode::faulty(5, vec![]),
            TestNode::faulty(5, vec![]),
        ],
        3,
    );

    run_test(nodes).await
}

#[tokio::test]
pub async fn discard_gossip_event_ok() {
    let nodes = Test::new(
        [
            TestNode::faulty(5, vec![Fault::DiscardGossipEvent(0.5)]),
            TestNode::faulty(15, vec![]),
            TestNode::faulty(15, vec![]),
        ],
        7,
    );

    run_test(nodes).await
}
