use std::net::{Ipv4Addr, SocketAddr};

use malachite_node::config::{Config, PeerConfig};
use malachite_node::network::PeerId;
use malachite_node::util::make_node;
use malachite_test::utils::make_validators;

use malachite_test::{Validator, ValidatorSet};
use tracing::info;

mod cli;
use cli::Cli;

const VOTING_PWERS: [u64; 3] = [5, 20, 10];

fn make_config<'a>(vs: impl Iterator<Item = &'a Validator>) -> Config {
    let peers = vs
        .enumerate()
        .map(|(i, v)| PeerConfig {
            id: PeerId::new(format!("node{}", i + 1)),
            addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 1200 + i as u16 + 1),
            public_key: v.public_key,
        })
        .collect();

    Config { peers }
}

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    tracing_subscriber::fmt::init();

    let args = Cli::from_env();

    // Validators keys are deterministic and match the ones in the config file
    let vs = make_validators(VOTING_PWERS);
    let config = make_config(vs.iter().map(|(v, _)| v));

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

    let mut handle = node.run().await;

    loop {
        if let Some((height, round, value)) = handle.wait_decision().await {
            info!(
                "[{}] Decision at height {height} and round {round}: {value:?}",
                args.peer_id
            );
        }
    }
}
