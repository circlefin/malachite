//! Connection flood stress test.
//!
//! This test verifies that a node handles many simultaneous connection attempts
//! gracefully without resource exhaustion.
//!
//! The `connection_limits::Behaviour` provides transport-level defense:
//! - Limits pending connections (handshake in progress)
//! - Limits established connections (hard cap before discovery logic)
//! - Rejects excess connections early, before consuming resources
//!
//! We verify this by counting TCP connections at the OS level.

use std::net::IpAddr;
use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_metrics::SharedRegistry;
use malachitebft_network::{
    spawn, ChannelNames, Config, DiscoveryConfig, GossipSubConfig, Keypair, ProtocolNames,
    PubSubProtocol,
};
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};

/// Count established TCP connections to a specific port.
/// Works cross-platform (Linux, macOS, Windows).
fn count_tcp_connections(port: u16) -> usize {
    let af_flags = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto_flags = ProtocolFlags::TCP;

    let sockets = match get_sockets_info(af_flags, proto_flags) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("Failed to get socket info: {}", e);
            return 0;
        }
    };

    sockets
        .into_iter()
        .filter(|s| {
            if let ProtocolSocketInfo::Tcp(tcp) = &s.protocol_socket_info {
                // Count only server-side sockets (incoming connections to target)
                // These have local_port = target port
                let is_incoming = tcp.local_port == port;
                let is_established = matches!(tcp.state, TcpState::Established);
                let is_localhost = match tcp.local_addr {
                    IpAddr::V4(ip) => ip.is_loopback(),
                    IpAddr::V6(ip) => ip.is_loopback(),
                };

                is_established && is_localhost && is_incoming
            } else {
                false
            }
        })
        .count()
}

fn init_logging() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
}

fn make_config(moniker: &str, port: u16, persistent_peers: Vec<u16>) -> Config {
    Config {
        moniker: moniker.to_string(),
        listen_addr: TransportProtocol::Tcp.multiaddr("127.0.0.1", port as usize),
        persistent_peers: persistent_peers
            .iter()
            .map(|p| TransportProtocol::Tcp.multiaddr("127.0.0.1", *p as usize))
            .collect(),
        discovery: DiscoveryConfig {
            enabled: false,
            num_inbound_peers: 3,
            num_outbound_peers: 3,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(60),
        transport: malachitebft_network::TransportProtocol::Tcp,
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

/// Tests that connection flood attack is mitigated.
#[tokio::test]
async fn connection_flood_attack() {
    init_logging();

    let base_port = 29400;
    let target_port = base_port;

    // Use transport limit > num_inbound_peers(3)
    // Use 100 peers to exceed the transport limit
    let num_peers = 100;
    let transport_limit = 12;

    // Target node
    let target_config = make_config("target", target_port, vec![]);
    let target_keypair = Keypair::generate_ed25519();
    let target_registry = SharedRegistry::global().with_moniker("flood-target");

    let target_handle = spawn(target_keypair, target_config, target_registry)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(300)).await;

    // Spawn many peers that all connect to the target
    let mut peer_handles = Vec::new();
    for i in 0..num_peers {
        let peer_port = base_port + 1 + i;
        let peer_config = make_config(&format!("peer-{}", i), peer_port, vec![target_port]);
        let peer_keypair = Keypair::generate_ed25519();
        let peer_registry = SharedRegistry::global().with_moniker(format!("flood-peer-{}", i));

        let handle = spawn(peer_keypair, peer_config, peer_registry)
            .await
            .unwrap();
        peer_handles.push(handle);
    }

    // Sample connection count during the attack
    // Wait briefly for connections to establish before sampling
    let mut max_tcp_connections = 0;
    for _ in 0..10 {
        let current = count_tcp_connections(target_port);
        max_tcp_connections = max_tcp_connections.max(current);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let tcp_connections = max_tcp_connections;

    tracing::info!(
        "TCP connections to port {}: {} (transport limit: {}, peers attempted: {})",
        target_port,
        tcp_connections,
        transport_limit,
        num_peers
    );

    assert!(
        tcp_connections <= transport_limit,
        "Connection limits not enforced: {} TCP connections exceeds transport limit of {}",
        tcp_connections,
        transport_limit
    );
    tracing::info!(
        "✓ Connection limits enforced: {} <= {}",
        tcp_connections,
        transport_limit
    );

    // Clean up
    for handle in peer_handles {
        drop(handle);
    }
    drop(target_handle);
}
