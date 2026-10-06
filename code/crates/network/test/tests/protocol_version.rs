use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_network::{
    spawn, Config, DiscoveryConfig, Event, Keypair, NetworkIdentity, ProtocolNames,
};
use tokio::time::sleep;

fn make_config(port: usize) -> Config {
    Config {
        listen_addr: TransportProtocol::Quic.multiaddr("127.0.0.1", port),
        persistent_peers: vec![],
        persistent_peers_only: false,
        discovery: DiscoveryConfig {
            enabled: false,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(60),
        transport: malachitebft_network::TransportProtocol::Quic,
        gossipsub: malachitebft_network::GossipSubConfig::default(),
        pubsub_protocol: malachitebft_network::PubSubProtocol::default(),
        channel_names: malachitebft_network::ChannelNames::default(),
        rpc_max_size: 10 * 1024 * 1024,
        pubsub_max_size: 4 * 1024 * 1024,
        pubsub_max_size_per_topic: Default::default(),
        sync_request_timeout: Duration::from_secs(10),
        sync_max_request_size: 1024 * 1024,
        sync_parallel_requests: 5,
        enable_consensus: true,
        enable_sync: false,
        protocol_names: ProtocolNames::default(),
    }
}

/// A peer whose Identify payload advertises a consensus protocol version other than
/// the locally configured one is disconnected, while a peer advertising the same
/// version connects normally.
#[tokio::test]
async fn incompatible_protocol_version_peer_is_disconnected() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let keypair3 = Keypair::generate_ed25519();
    let base_port = 40000;

    let node1_libp2p_peer_id = keypair1.public().to_peer_id();
    let node1_addr: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port, node1_libp2p_peer_id
    )
    .parse()
    .unwrap();

    // Node1: observer, default protocol names.
    let config1 = make_config(base_port);

    // Node2: same protocol version as node1 — must be accepted.
    let mut config2 = make_config(base_port + 1);
    config2.persistent_peers = vec![node1_addr.clone()];

    // Node3: different consensus protocol version — must be disconnected.
    let mut config3 = make_config(base_port + 2);
    config3.persistent_peers = vec![node1_addr];
    config3.protocol_names.consensus = "/malachitebft-core-consensus/v2beta1".to_string();

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1-vsnpr".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        config2,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2-vsnpr".to_string()),
    )
    .await
    .unwrap();

    let mut handle3 = spawn(
        NetworkIdentity::new(
            "node-3".to_string(),
            keypair3,
            Some("test-address-3".to_string()),
        ),
        config3,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-3-vsnpr".to_string()),
    )
    .await
    .unwrap();

    let node1_peer_id = handle1.peer_id();
    let node2_peer_id = handle2.peer_id();
    let node3_peer_id = handle3.peer_id();

    let mut node2_connected = false;
    let mut node3_connected = false;
    let mut node3_disconnected = false;
    let mut node1_connected_on_node3 = false;
    let mut node1_disconnected_on_node3 = false;
    for _ in 0..100 {
        tokio::select! {
            event = handle1.recv() => {
                match event {
                    Some(Event::PeerConnected(peer_id)) if peer_id == node2_peer_id => {
                        node2_connected = true;
                    }
                    Some(Event::PeerConnected(peer_id)) if peer_id == node3_peer_id => {
                        node3_connected = true;
                    }
                    Some(Event::PeerDisconnected(peer_id)) if peer_id == node3_peer_id => {
                        node3_disconnected = true;
                    }
                    _ => {}
                }
            }
            event = handle3.recv() => {
                match event {
                    Some(Event::PeerConnected(peer_id)) if peer_id == node1_peer_id => {
                        node1_connected_on_node3 = true;
                    }
                    Some(Event::PeerDisconnected(peer_id)) if peer_id == node1_peer_id => {
                        node1_disconnected_on_node3 = true;
                    }
                    _ => {}
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
        if node2_connected && node3_disconnected && node1_disconnected_on_node3 {
            break;
        }
    }

    assert!(
        node2_connected,
        "Node2 (matching protocol version) should connect"
    );
    assert!(
        node3_disconnected,
        "Node3 (mismatched protocol version) should be disconnected"
    );
    assert!(
        !node3_connected,
        "Node3 (mismatched protocol version) should never be reported as connected"
    );
    assert!(
        node1_disconnected_on_node3,
        "Node3 should also tear down the connection to node1"
    );
    assert!(
        !node1_connected_on_node3,
        "Node3 should never report node1 as connected"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
    handle3.shutdown().await.unwrap();
}

fn init_logging() {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let filter = EnvFilter::builder()
        .parse("info,arc_malachitebft=debug,ractor=error")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_thread_ids(false);

    let _ = builder.finish().try_init();
}
