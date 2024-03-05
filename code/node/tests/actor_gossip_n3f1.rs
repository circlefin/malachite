use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use malachite_common::Round;
use malachite_node::actors::faulty_node::{Fault, FaultyNode};
use malachite_node::actors::node::Msg;
use malachite_node::util::make_node_actor;
use malachite_test::utils::make_validators;
use malachite_test::{Height, ValidatorSet, Value};
use ractor::Actor;
use rand::SeedableRng;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::info;

#[tokio::test]
#[should_panic(expected = "Not all nodes made correct decisions: 7/9")]
pub async fn decide_on_value() {
    tracing_subscriber::fmt::init();

    let voting_powers = [5, 15, 10];
    let faulty_node = 0;

    // Validators keys are deterministic and match the ones in the config file
    let vals_and_keys = make_validators(voting_powers);
    let vs = ValidatorSet::new(vals_and_keys.iter().map(|(v, _)| v.clone()));

    let mut handles = Vec::with_capacity(vals_and_keys.len());

    for (v, sk) in vals_and_keys {
        let (tx_decision, rx_decision) = mpsc::channel(32);
        let node = tokio::spawn(make_node_actor(vs.clone(), sk, v.address, tx_decision));
        handles.push((node, rx_decision));
    }

    sleep(Duration::from_secs(5)).await;

    let mut nodes = Vec::with_capacity(handles.len());

    for (handle, rx) in handles {
        let node = handle.await.expect("Error: node failed to start");
        nodes.push((node, rx));
    }

    let mut handles = Vec::with_capacity(nodes.len());

    for (i, (node, rx)) in nodes.into_iter().enumerate() {
        let handle = if i == faulty_node {
            let faults = vec![Fault::DiscardGossipEvent(0.5)];
            let rng = Box::new(rand::rngs::StdRng::seed_from_u64(42));

            FaultyNode::spawn(node, faults, rng).await.unwrap()
        } else {
            Actor::spawn(None, node, ()).await.unwrap().0
        };

        handle.cast(Msg::StartHeight(Height::new(1))).unwrap();
        handles.push((handle, rx));
    }

    let correct_decisions = Arc::new(AtomicUsize::new(0));

    let (handles, rxs): (Vec<_>, Vec<_>) = handles.into_iter().unzip();

    for (i, mut rx_decision) in rxs.into_iter().enumerate() {
        let correct_decisions = Arc::clone(&correct_decisions);

        tokio::spawn(async move {
            for height in 1..=3 {
                let decision = rx_decision.recv().await;

                assert_eq!(
                    decision,
                    Some((Height::new(height), Round::new(0), Value::new(40 + height)))
                );

                info!("[{height}] {i}/3 correct decision");

                correct_decisions.fetch_add(1, Ordering::Relaxed);
            }
        });
    }

    tokio::time::sleep(Duration::from_secs(20)).await;

    let expected_decisions = handles.len() * 3;
    let correct_decisions = correct_decisions.load(Ordering::Relaxed);

    if correct_decisions != expected_decisions {
        for handle in &handles {
            handle.stop_and_wait(None, None).await.unwrap();
        }

        panic!(
            "Not all nodes made correct decisions: {}/{}",
            correct_decisions, expected_decisions
        );
    }

    for handle in handles {
        handle.stop_and_wait(None, None).await.unwrap();
    }
}
