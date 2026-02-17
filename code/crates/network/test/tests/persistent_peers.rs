use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_network::{
    spawn, Config, DiscoveryConfig, Event, Keypair, Multiaddr, NetworkIdentity,
    PersistentPeerError, ProtocolNames,
};
use tokio::time::sleep;

/// Build a Quic multiaddr with PeerId (required for persistent peers).
fn quic_multiaddr_with_peer_id(host: &str, port: usize, peer_id: impl std::fmt::Display) -> Multiaddr {
    format!("/ip4/{host}/udp/{port}/quic-v1/p2p/{peer_id}")
        .parse()
        .expect("valid multiaddr")
}

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
        enable_consensus: true,
        enable_sync: false,
        protocol_names: ProtocolNames::default(),
    }
}

/// Test adding and removing persistent peers at runtime, including edge cases
#[tokio::test]
async fn test_add_and_remove_persistent_peer() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let node2_peer_id = keypair2.public().to_peer_id();
    let base_port = 31000;

    let handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        make_config(base_port),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        make_config(base_port + 1),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;
    let node2_addr = quic_multiaddr_with_peer_id("127.0.0.1", base_port + 1, node2_peer_id);
    let non_existent_peer_id = Keypair::generate_ed25519().public().to_peer_id();
    let non_existent_addr =
        quic_multiaddr_with_peer_id("127.0.0.1", base_port + 100, non_existent_peer_id);

    // Remove non-existent peer returns NotFound
    let result = handle1
        .remove_persistent_peer(non_existent_addr)
        .await
        .unwrap();
    assert_eq!(result, Err(PersistentPeerError::NotFound));

    // Add peer succeeds
    let result = handle1
        .add_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Adding same peer again returns AlreadyExists
    let result = handle1
        .add_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Err(PersistentPeerError::AlreadyExists));

    // Remove peer succeeds
    let result = handle1
        .remove_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Removing same peer again returns NotFound
    let result = handle1.remove_persistent_peer(node2_addr).await.unwrap();
    assert_eq!(result, Err(PersistentPeerError::NotFound));

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// Test that adding a persistent peer establishes a connection
#[tokio::test]
async fn test_persistent_peer_establishes_connection() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let node2_peer_id = keypair2.public().to_peer_id();
    let base_port = 32000;

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        make_config(base_port),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        make_config(base_port + 1),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    // Add peer and verify connection is established
    let node2_addr = quic_multiaddr_with_peer_id("127.0.0.1", base_port + 1, node2_peer_id);
    let result = handle1.add_persistent_peer(node2_addr).await.unwrap();
    assert_eq!(result, Ok(()));

    // Wait for PeerConnected event
    let mut connected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if let Some(Event::PeerConnected(_)) = event {
                    connected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(connected, "Persistent peer should connect");

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// Test removing a peer while a dial is in progress
#[tokio::test]
async fn test_remove_peer_during_dial() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let base_port = 33000;

    let handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        make_config(base_port),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    // Add a persistent peer to a non-existent/unreachable address
    // This will start a dial attempt that will fail
    let unreachable_peer_id = Keypair::generate_ed25519().public().to_peer_id();
    let unreachable_addr =
        quic_multiaddr_with_peer_id("127.0.0.1", base_port + 50, unreachable_peer_id);
    let result = handle1
        .add_persistent_peer(unreachable_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Immediately remove the peer while dial is in progress
    // This should succeed even though the dial hasn't completed
    sleep(Duration::from_millis(50)).await;
    let result = handle1
        .remove_persistent_peer(unreachable_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Try removing again - should return NotFound
    let result = handle1
        .remove_persistent_peer(unreachable_addr)
        .await
        .unwrap();
    assert_eq!(result, Err(PersistentPeerError::NotFound));

    handle1.shutdown().await.unwrap();
}

/// Test removing a peer while connected in persistent_peers_only mode
#[tokio::test]
async fn test_remove_connected_peer_in_persistent_only_mode() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let node2_peer_id = keypair2.public().to_peer_id();
    let base_port = 34000;

    let mut config1 = make_config(base_port);
    config1.persistent_peers_only = true;

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        make_config(base_port + 1),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    // Add peer and wait for connection
    let node2_addr = quic_multiaddr_with_peer_id("127.0.0.1", base_port + 1, node2_peer_id);
    let result = handle1
        .add_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Wait for PeerConnected event
    let mut connected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if let Some(Event::PeerConnected(_)) = event {
                    connected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(connected, "Persistent peer should connect");

    // Now remove the peer while connected
    let result = handle1
        .remove_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Verify the peer is no longer in persistent peers by trying to remove again
    let result = handle1.remove_persistent_peer(node2_addr).await.unwrap();
    assert_eq!(result, Err(PersistentPeerError::NotFound));

    // In persistent_peers_only mode, removing a peer should disconnect it.
    // Wait for PeerDisconnected event to verify this behavior.
    let mut disconnected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if let Some(Event::PeerDisconnected(_)) = event {
                    disconnected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(
        disconnected,
        "Peer should be disconnected after removal in persistent_peers_only mode"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// Test race between add/remove and periodic dial_bootstrap_nodes
#[tokio::test]
async fn test_add_remove_race_with_periodic_dial() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 35000;

    let node2_peer_id = keypair2.public().to_peer_id();
    let node2_addr = quic_multiaddr_with_peer_id("127.0.0.1", base_port + 1, node2_peer_id);

    // Initialize node1 with node2 in persistent_peers to ensure
    // the periodic dial_bootstrap_nodes task is actively running
    let mut config1 = make_config(base_port);
    config1.persistent_peers = vec![node2_addr.clone()];

    let handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        make_config(base_port + 1),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    // Now rapidly add and remove the peer multiple times to create race conditions
    // with the periodic dial_bootstrap_nodes task that's already running
    for _ in 0..10 {
        // Remove the peer (it's already in the list from config)
        let result = handle1
            .remove_persistent_peer(node2_addr.clone())
            .await
            .unwrap();
        // Should succeed or return NotFound if already removed in a previous iteration
        assert!(
            result == Ok(()) || result == Err(PersistentPeerError::NotFound),
            "Remove should succeed or return NotFound, got {:?}",
            result
        );

        // Small delay to allow periodic dial to potentially trigger
        sleep(Duration::from_millis(10)).await;

        // Add the peer back
        let result = handle1
            .add_persistent_peer(node2_addr.clone())
            .await
            .unwrap();
        // Should succeed or return AlreadyExists if already added
        assert!(
            result == Ok(()) || result == Err(PersistentPeerError::AlreadyExists),
            "Add should succeed or return AlreadyExists, got {:?}",
            result
        );

        sleep(Duration::from_millis(10)).await;
    }

    // Final remove and verify system is still functional
    let result = handle1
        .remove_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert!(
        result == Ok(()) || result == Err(PersistentPeerError::NotFound),
        "Final remove should succeed or return NotFound, got {:?}",
        result
    );

    // Add back and verify operations still work correctly
    let result = handle1.add_persistent_peer(node2_addr).await.unwrap();
    assert_eq!(result, Ok(()));

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// Test that spawn fails when persistent_peers contains an address without PeerId
#[tokio::test]
async fn test_spawn_rejects_persistent_peers_without_peer_id() {
    init_logging();

    let keypair = Keypair::generate_ed25519();
    let mut config = make_config(36000);
    // Address without /p2p/<peer_id> must be rejected at startup
    config.persistent_peers = vec!["/ip4/127.0.0.1/udp/36001/quic-v1"
        .parse()
        .expect("valid multiaddr")];

    let result = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair,
            Some("test-address-1".to_string()),
        ),
        config,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await;

    let err = match result {
        Ok(_) => panic!("expected spawn to fail with persistent_peers without PeerId"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains("missing PeerId"),
        "expected 'missing PeerId' in error: {}",
        err
    );
}

/// Test that a connection is rejected when the remote peer's PeerId does not match
/// the expected PeerId in the persistent peer address (identity verification).
#[tokio::test]
async fn test_persistent_peer_identity_verification_rejects_mismatch() {
    init_logging();

    let keypair_a = Keypair::generate_ed25519();
    let keypair_b = Keypair::generate_ed25519(); // A expects this PeerId at the address
    let keypair_c = Keypair::generate_ed25519(); // C will actually listen on the address
    let base_port = 37500;

    // A's config: persistent peer at C's port but with B's PeerId (B is never started)
    let peer_id_b = keypair_b.public().to_peer_id();
    let addr_expecting_b = quic_multiaddr_with_peer_id("127.0.0.1", base_port + 1, peer_id_b);

    let mut config_a = make_config(base_port);
    config_a.persistent_peers = vec![addr_expecting_b];
    config_a.persistent_peers_only = true;

    let mut handle_a = spawn(
        NetworkIdentity::new(
            "node-a".to_string(),
            keypair_a,
            Some("test-address-a".to_string()),
        ),
        config_a,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-a".to_string()),
    )
    .await
    .unwrap();

    let handle_c = spawn(
        NetworkIdentity::new(
            "node-c".to_string(),
            keypair_c,
            Some("test-address-c".to_string()),
        ),
        make_config(base_port + 1),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-c".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    // A will dial the persistent peer addr (port of C, but expected PeerId is B).
    // A connects to C; after Identify, A sees C's PeerId != B's -> must reject and disconnect.
    // So A should never report PeerConnected for C (or connection drops quickly).
    let mut connected_peers = 0u32;
    for _ in 0..30 {
        tokio::select! {
            event = handle_a.recv() => {
                match event {
                    Some(Event::PeerConnected(_)) => connected_peers += 1,
                    Some(_) => {}
                    None => break,
                }
            }
            _ = sleep(Duration::from_millis(100)) => break,
        }
    }

    assert_eq!(
        connected_peers, 0,
        "Identity verification should reject peer with mismatched PeerId (no PeerConnected)"
    );

    handle_a.shutdown().await.unwrap();
    handle_c.shutdown().await.unwrap();
}

/// Test that add_persistent_peer returns PeerIdRequired when address has no PeerId
#[tokio::test]
async fn test_add_persistent_peer_requires_peer_id() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let base_port = 37000;

    let handle = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        make_config(base_port),
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1".to_string()),
    )
    .await
    .unwrap();

    let addr_without_peer_id: Multiaddr =
        "/ip4/127.0.0.1/udp/37001/quic-v1".parse().expect("valid multiaddr");

    let result = handle.add_persistent_peer(addr_without_peer_id).await.unwrap();
    assert_eq!(result, Err(PersistentPeerError::PeerIdRequired));

    handle.shutdown().await.unwrap();
}

fn init_logging() {
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, FmtSubscriber};

    let filter = EnvFilter::builder()
        .parse("info,informalsystems_malachitebft=debug,ractor=error")
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let builder = FmtSubscriber::builder()
        .with_target(false)
        .with_env_filter(filter)
        .with_writer(std::io::stdout)
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_thread_ids(false);

    let _ = builder.finish().try_init();
}
