use malachite_common::Round;
use malachite_node::actors::node::Msg;
use malachite_node::util::make_node_actor;
use malachite_test::utils::make_validators;
use malachite_test::{Height, ValidatorSet, Value};
use ractor::Actor;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::info;

#[tokio::test]
pub async fn decide_on_value() {
    tracing_subscriber::fmt::init();

    let voting_powers = [5, 20, 10];

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

    for (node, rx) in nodes {
        let (handle, _) = Actor::spawn(None, node, ()).await.unwrap();
        handle.cast(Msg::StartHeight(Height::new(1))).unwrap();
        handles.push((handle, rx));
    }

    for height in 1..=3 {
        let mut i = 0;

        for (_, rx_decision) in &mut handles {
            i += 1;

            let decision = rx_decision.recv().await;

            assert_eq!(
                decision,
                Some((Height::new(height), Round::new(0), Value::new(40 + height)))
            );

            info!("[{i}/3] Correct decision at height {}", height);
        }
    }

    for (handle, _) in handles {
        handle.stop_and_wait(None, None).await.unwrap();
    }
}
