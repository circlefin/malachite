//! Per-IP connection limit tests.
//!
//! Tests that the ip_limits behaviour correctly limits inbound connections
//! from the same IP address.

use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_metrics::SharedRegistry;
use malachitebft_network::{
    spawn, ChannelNames, Config, DiscoveryConfig, GossipSubConfig, Keypair, NetworkIdentity,
    ProtocolNames, PubSubProtocol,
};

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("debug")
        .try_init();
}

fn make_config(port: u16, persistent_peers: Vec<u16>, max_connections_per_ip: usize) -> Config {
    Config {
        listen_addr: TransportProtocol::Quic.multiaddr("127.0.0.1", port as usize),
        persistent_peers: persistent_peers
            .iter()
            .map(|p| TransportProtocol::Quic.multiaddr("127.0.0.1", *p as usize))
            .collect(),
        discovery: DiscoveryConfig {
            enabled: false,
            num_inbound_peers: 10,
            num_outbound_peers: 10,
            max_connections_per_ip,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(60),
        transport: malachitebft_network::TransportProtocol::Quic,
        gossipsub: GossipSubConfig::default(),
        pubsub_protocol: PubSubProtocol::default(),
        channel_names: ChannelNames::default(),
        rpc_max_size: 10 * 1024 * 1024,
        pubsub_max_size: 4 * 1024 * 1024,
        enable_consensus: true,
        enable_sync: false,
        protocol_names: ProtocolNames::default(),
        persistent_peers_only: false,
    }
}

/// Tests that attack by a flood of connections from the same IP address is mitigated.
#[tokio::test]
async fn same_ip_connection_attack() {
    init_logging();

    let base_port: u16 = rand::random::<u16>() % 10000 + 30000;
    let target_port = base_port;

    let max_connections_per_ip = 2;
    let num_peers = 5; // More than the limit

    // Target node with low per-IP limit
    let target_config = make_config(target_port, vec![], max_connections_per_ip);
    let target_keypair = Keypair::generate_ed25519();
    let target_identity = NetworkIdentity::new("target".to_string(), target_keypair, None);
    let target_registry = SharedRegistry::global().with_moniker("ip-limit-target");

    let mut target_handle = spawn(target_identity, target_config, target_registry)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Spawn peers that all connect to the target from the same IP (127.0.0.1)
    let mut peer_handles = Vec::new();
    for i in 0..num_peers {
        let peer_port = base_port + 1 + i;
        // Peers use default (no per-IP limit)
        let peer_config = make_config(peer_port, vec![target_port], 100);
        let peer_keypair = Keypair::generate_ed25519();
        let peer_identity = NetworkIdentity::new(format!("peer-{}", i), peer_keypair, None);
        let peer_registry = SharedRegistry::global().with_moniker(format!("ip-limit-peer-{}", i));

        let handle = spawn(peer_identity, peer_config, peer_registry)
            .await
            .unwrap();
        peer_handles.push(handle);
    }

    // Wait for connection attempts and stabilization
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Count connected peers on target by receiving PeerConnected events
    let mut connected_peers = 0;
    loop {
        tokio::select! {
            event = target_handle.recv() => {
                match event {
                    Some(malachitebft_network::Event::PeerConnected(_)) => {
                        connected_peers += 1;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                break;
            }
        }
    }

    tracing::info!(
        "Connected peers: {} (expected max: {}, attempted: {})",
        connected_peers,
        max_connections_per_ip,
        num_peers
    );

    assert!(
        connected_peers <= max_connections_per_ip,
        "Per-IP limit not enforced: {} connections exceeds limit of {}",
        connected_peers,
        max_connections_per_ip
    );

    assert!(connected_peers > 0, "No peers connected - test setup issue");

    tracing::info!(
        "✓ Per-IP limit enforced: {} <= {}",
        connected_peers,
        max_connections_per_ip
    );

    // Clean up
    for handle in peer_handles {
        drop(handle);
    }
    drop(target_handle);
}
