use bytesize::ByteSize;

use malachite_node::config::{
    App, Config as NodeConfig, ConsensusConfig, LoggingConfig, MempoolConfig, MetricsConfig,
    P2pConfig, RuntimeConfig, TimeoutConfig,
};

use crate::utils::test::Test;

pub fn make_node_config<const N: usize>(test: &Test<N>, i: usize, app: App) -> NodeConfig {
    NodeConfig {
        app,
        moniker: format!("node-{i}"),
        consensus: ConsensusConfig {
            max_block_size: ByteSize::mib(1),
            timeouts: TimeoutConfig::default(),
            p2p: P2pConfig {
                listen_addr: format!(
                    "/ip4/127.0.0.1/udp/{}/quic-v1",
                    test.consensus_base_port + i
                )
                .parse()
                .unwrap(),
                persistent_peers: (0..N)
                    .filter(|j| i != *j)
                    .map(|j| {
                        format!(
                            "/ip4/127.0.0.1/udp/{}/quic-v1",
                            test.consensus_base_port + j
                        )
                        .parse()
                        .unwrap()
                    })
                    .collect(),
            },
        },
        mempool: MempoolConfig {
            p2p: P2pConfig {
                listen_addr: format!("/ip4/127.0.0.1/udp/{}/quic-v1", test.mempool_base_port + i)
                    .parse()
                    .unwrap(),
                persistent_peers: (0..N)
                    .filter(|j| i != *j)
                    .map(|j| {
                        format!("/ip4/127.0.0.1/udp/{}/quic-v1", test.mempool_base_port + j)
                            .parse()
                            .unwrap()
                    })
                    .collect(),
            },
            max_tx_count: 10000,
            gossip_batch_size: 100,
        },
        metrics: MetricsConfig {
            enabled: false,
            listen_addr: format!("127.0.0.1:{}", test.metrics_base_port + i)
                .parse()
                .unwrap(),
        },
        logging: LoggingConfig::default(),
        runtime: RuntimeConfig::single_threaded(),
        test: Default::default(),
    }
}
