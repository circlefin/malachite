use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_network::{
    spawn, Config, DiscoveryConfig, Event, Keypair, NetworkIdentity, PersistentPeerError,
    ProtocolNames,
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

    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 1);
    let non_existent_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 100);

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
    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 1);
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
    let unreachable_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 50);
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
    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 1);
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

    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 1);

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
