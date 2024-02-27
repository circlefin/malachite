use malachite_common::Round;
use malachite_node::util::make_gossip_node;
use malachite_test::utils::make_validators;
use malachite_test::{Height, ValidatorSet, Value};
use tokio::time::{sleep, Duration};

#[tokio::test]
pub async fn decide_on_value() {
    tracing_subscriber::fmt::init();

    let voting_powers = [5, 20, 10, 30, 15, 1, 5, 25, 10, 15];

    // Validators keys are deterministic and match the ones in the config file
    let vals_and_keys = make_validators(voting_powers);
    let vs = ValidatorSet::new(vals_and_keys.iter().map(|(v, _)| v.clone()));

    let mut handles = Vec::with_capacity(vals_and_keys.len());

    for (v, sk) in vals_and_keys {
        let node = tokio::spawn(make_gossip_node(vs.clone(), sk, v.address));
        handles.push(node);
    }

    sleep(Duration::from_secs(5)).await;

    let mut nodes = Vec::with_capacity(handles.len());

    for handle in handles {
        let node = handle.await.expect("Error: node failed to start");
        nodes.push(node);
    }

    let mut handles = Vec::with_capacity(nodes.len());

    for node in nodes {
        let handle = node.run().await;
        handles.push(handle);
    }

    for height in 1..=3 {
        for handle in &mut handles {
            let decision = handle.wait_decision().await;

            assert_eq!(
                decision,
                Some((Height::new(height), Round::new(0), Value::new(40 + height)))
            );
        }
    }

    for handle in handles {
        handle.abort();
    }
}
