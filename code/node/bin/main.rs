use std::sync::Arc;
use std::time::Duration;

use malachite_node::config::Config;
use malachite_node::network::broadcast;
use malachite_node::network::broadcast::PeerInfo;
use malachite_node::node::{Node, Params};
use malachite_node::peers::Peers;
use malachite_node::timers;
use malachite_test::utils::{make_validators, FixedProposer};
use malachite_test::{Address, Height, PrivateKey, TestContext, ValidatorSet, Value};
use tracing::info;

mod cli;
use cli::Cli;

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    tracing_subscriber::fmt::init();

    let args = Cli::from_env();

    // Validators keys are deterministic and match the ones in the config file
    let vs = make_validators([2, 3, 2]);

    let config = std::fs::read_to_string("node/peers.toml").expect("Error: missing peers.toml");
    let config = toml::from_str::<Config>(&config).expect("Error: invalid peers.toml");

    let peer_config = config
        .peers
        .iter()
        .find(|p| p.id == args.peer_id)
        .expect("Error: invalid peer id");

    let (my_sk, my_addr) = vs
        .iter()
        .find(|(v, _)| v.public_key == peer_config.public_key)
        .map(|(v, pk)| (pk.clone(), v.address))
        .expect("Error: invalid peer id");

    let (vs, _): (Vec<_>, Vec<_>) = vs.into_iter().unzip();

    let peer_info = peer_config.peer_info();
    let vs = ValidatorSet::new(vs);

    let node = make_node(vs, my_sk, my_addr, peer_info, config.into()).await;

    info!("[{}] Starting...", args.peer_id);

    node.run().await;
}

pub async fn make_node(
    validator_set: ValidatorSet,
    private_key: PrivateKey,
    address: Address,
    peer_info: PeerInfo,
    peers: Peers<TestContext>,
) -> Node<TestContext, broadcast::Handle> {
    let start_height = Height::new(1);
    let ctx = TestContext::new(private_key);
    let proposer_selector = Arc::new(FixedProposer::new(validator_set.validators[0].address));

    let params = Params {
        start_height,
        proposer_selector,
        validator_set,
        address,
        peers: peers.clone(),
        threshold_params: Default::default(),
    };

    let timers_config = timers::Config {
        propose_timeout: Duration::from_secs(10),
        prevote_timeout: Duration::from_secs(5),
        precommit_timeout: Duration::from_secs(5),
        commit_timeout: Duration::from_secs(5),
    };

    let network = broadcast::Peer::new(peer_info.clone());
    let handle = network.run().await;

    let timeout = Some(Duration::from_secs(10));

    let to_connect = peers
        .iter()
        .filter(|p| p.id != peer_info.id)
        .map(|p| p.peer_info());

    for peer in to_connect {
        handle.connect_to_peer(peer, timeout).await;
    }

    Node::new(ctx, params, handle, Value::new(42), timers_config)
}
