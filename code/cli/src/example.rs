/// Example configurations for testing with indexed validators
use malachite_node::config::{Config, ConsensusConfig, MempoolConfig, P2pConfig, TimeoutConfig};
use malachite_test::ValidatorSet as Genesis;
use malachite_test::{PrivateKey, Validator};
use rand::prelude::StdRng;
use rand::SeedableRng;

const CONSENSUS_BASE_PORT: usize = 27000;
const MEMPOOL_BASE_PORT: usize = 28000;

/// Generate example configuration
pub fn generate_config(index: usize) -> Config {
    let consensus_port = CONSENSUS_BASE_PORT + index;
    let mempool_port = MEMPOOL_BASE_PORT + index;

    Config {
        moniker: format!("test-{}", index),
        consensus: ConsensusConfig {
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                listen_addr: format!("/ip4/127.0.0.1/udp/{consensus_port}/quic-v1")
                    .parse()
                    .unwrap(),
                persistent_peers: (0..3)
                    .filter(|j| *j != index)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", CONSENSUS_BASE_PORT + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                listen_addr: format!("/ip4/127.0.0.1/udp/{mempool_port}/quic-v1")
                    .parse()
                    .unwrap(),
                persistent_peers: (0..3)
                    .filter(|j| *j != index)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", MEMPOOL_BASE_PORT + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
        },
    }
}

/// Generate an example genesis configuration
pub fn generate_genesis() -> Genesis {
    let voting_power = vec![11, 10, 10];

    let mut rng = StdRng::seed_from_u64(0x42);
    let mut validators = Vec::with_capacity(voting_power.len());

    for vp in voting_power {
        validators.push(Validator::new(
            PrivateKey::generate(&mut rng).public_key(),
            vp,
        ));
    }

    Genesis { validators }
}

/// Generate an example private key
pub fn generate_private_key(index: usize) -> PrivateKey {
    let mut rng = StdRng::seed_from_u64(0x42);
    for _ in 0..index {
        let _ = PrivateKey::generate(&mut rng);
    }
    PrivateKey::generate(&mut rng)
}
