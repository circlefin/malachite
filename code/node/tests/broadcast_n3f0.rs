use malachite_common::Round;
use malachite_node::config::Config;
use malachite_node::util::make_node;
use malachite_test::utils::make_validators;
use malachite_test::{Height, ValidatorSet, Value};

#[tokio::test]
pub async fn decide_on_value() {
    tracing_subscriber::fmt::init();

    // Validators keys are deterministic and match the ones in the config file
    let vs = make_validators([2, 3, 2]);

    let config = include_str!("../peers.toml");
    let config = toml::from_str::<Config>(config).expect("Error: invalid peers.toml");

    let mut handles = Vec::with_capacity(config.peers.len());

    for peer_config in &config.peers {
        let (my_sk, my_addr) = vs
            .iter()
            .find(|(v, _)| v.public_key == peer_config.public_key)
            .map(|(v, pk)| (pk.clone(), v.address))
            .expect("Error: invalid peer id");

        let (vs, _): (Vec<_>, Vec<_>) = vs.clone().into_iter().unzip();

        let peer_info = peer_config.peer_info();
        let vs = ValidatorSet::new(vs);

        let node = tokio::spawn(make_node(
            vs,
            my_sk,
            my_addr,
            peer_info,
            config.clone().into(),
        ));

        handles.push(node);
    }

    let mut nodes = Vec::with_capacity(config.peers.len());

    for handle in handles {
        let node = handle.await.expect("Error: node failed to start");
        nodes.push(node);
    }

    let mut handles = Vec::with_capacity(config.peers.len());

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
