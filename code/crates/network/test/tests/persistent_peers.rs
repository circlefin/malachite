use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_network::{
    spawn, Config, DiscoveryConfig, Event, Keypair, NetworkIdentity, PeerIdExt,
    PersistentPeerError, ProtocolNames,
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

#[tokio::test]
async fn remove_address_only_persistent_peer_before_first_connection() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", 33501);

    let mut config1 = make_config(33500);
    config1.persistent_peers = vec![node2_addr.clone()];

    let handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global()
            .with_moniker("node-1-remove-before-connect".to_string()),
    )
    .await
    .unwrap();

    let dump = handle1.dump_state().await.unwrap();
    assert!(dump.persistent_peer_ids.is_empty());
    assert!(dump.persistent_peer_addrs.contains(&node2_addr));

    let result = handle1
        .remove_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    let dump = handle1.dump_state().await.unwrap();
    assert!(dump.persistent_peer_ids.is_empty());
    assert!(!dump.persistent_peer_addrs.contains(&node2_addr));

    handle1.shutdown().await.unwrap();
}

#[tokio::test]
async fn remove_address_only_persistent_peer_after_disconnect_clears_peer_id_on_reconnect() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 33510;

    let node1_peer_id = keypair1.public().to_peer_id();
    let node1_addr: malachitebft_network::Multiaddr =
        format!("/ip4/127.0.0.1/udp/{base_port}/quic-v1/p2p/{node1_peer_id}")
            .parse()
            .unwrap();
    let node2_peer_id = keypair2.public().to_peer_id();
    let node2_addr = TransportProtocol::Quic.multiaddr("127.0.0.1", base_port + 1);

    let mut config1 = make_config(base_port);
    config1.persistent_peers = vec![node2_addr.clone()];
    let config2 = make_config(base_port + 1);
    let mut reconnect_config = make_config(base_port + 2);
    reconnect_config.persistent_peers = vec![node1_addr];

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global()
            .with_moniker("node-1-remove-after-disconnect".to_string()),
    )
    .await
    .unwrap();

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2.clone(),
            Some("test-address-2".to_string()),
        ),
        config2.clone(),
        malachitebft_metrics::SharedRegistry::global()
            .with_moniker("node-2-remove-after-disconnect".to_string()),
    )
    .await
    .unwrap();

    let mut connected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if matches!(event, Some(Event::PeerConnected(peer_id)) if peer_id.to_libp2p() == node2_peer_id) {
                    connected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }
    assert!(connected, "Persistent peer should connect");

    handle2.wait_shutdown().await.unwrap();

    let mut disconnected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if matches!(event, Some(Event::PeerDisconnected(peer_id)) if peer_id.to_libp2p() == node2_peer_id) {
                    disconnected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }
    assert!(disconnected, "Persistent peer should disconnect");

    let result = handle1
        .remove_persistent_peer(node2_addr.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    let dump = handle1.dump_state().await.unwrap();
    assert!(!dump.persistent_peer_ids.contains(&node2_peer_id));
    assert!(!dump.persistent_peer_addrs.contains(&node2_addr));

    let handle2 = spawn(
        NetworkIdentity::new(
            "node-2".to_string(),
            keypair2,
            Some("test-address-2".to_string()),
        ),
        reconnect_config,
        malachitebft_metrics::SharedRegistry::global()
            .with_moniker("node-2-reconnect-after-removal".to_string()),
    )
    .await
    .unwrap();

    let mut reconnected = false;
    for _ in 0..50 {
        tokio::select! {
            event = handle1.recv() => {
                if matches!(event, Some(Event::PeerConnected(peer_id)) if peer_id.to_libp2p() == node2_peer_id) {
                    reconnected = true;
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }
    assert!(
        reconnected,
        "Removed peer should reconnect as an inbound peer"
    );

    let dump = handle1.dump_state().await.unwrap();
    assert!(
        !dump
            .peers
            .get(&node2_peer_id)
            .expect("reconnected peer should be in peer_info")
            .peer_type
            .is_persistent(),
        "Removed peer should not be classified as persistent after reconnecting"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
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

/// When explicit peering is enabled, a runtime-added persistent peer should
/// join the gossipsub explicit-peer set as soon as the connection is
/// established — not wait for some later event.
#[tokio::test]
async fn test_runtime_add_marks_peer_explicit_when_explicit_peering_enabled() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 36000;

    let mut config1 = make_config(base_port);
    config1.gossipsub.enable_explicit_peering = true;
    let mut config2 = make_config(base_port + 1);
    config2.gossipsub.enable_explicit_peering = true;

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
        config2,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(500)).await;

    let node2_libp2p_peer_id = handle2.peer_id().to_libp2p();
    let node2_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port + 1,
        node2_libp2p_peer_id
    )
    .parse()
    .unwrap();

    let result = handle1
        .add_persistent_peer(node2_addr_with_p2p.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    // Wait for PeerConnected so peer_info is populated and the Identify
    // handler has classified the peer as persistent + explicit.
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

    // Give the Identify handler a moment to finalize state after PeerConnected.
    sleep(Duration::from_millis(200)).await;

    let dump = handle1.dump_state().await.unwrap();
    let peer_info = dump
        .peers
        .get(&node2_libp2p_peer_id)
        .expect("node-2 should be in peer_info");
    assert!(
        peer_info.is_explicit,
        "runtime-added persistent peer should be in gossipsub explicit set"
    );
    assert!(
        peer_info.peer_type.is_persistent(),
        "runtime-added peer should be classified as persistent"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// When explicit peering is enabled, removing a persistent peer that stays
/// connected (e.g. inbound-only) must clear the gossipsub explicit-peer
/// set immediately. The ConnectionClosed fallback never fires while the
/// connection is up.
#[tokio::test]
async fn test_runtime_remove_clears_explicit_when_still_connected() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 37000;

    let node1_libp2p_peer_id = keypair1.public().to_peer_id();
    let node1_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port, node1_libp2p_peer_id
    )
    .parse()
    .unwrap();

    // Node 1 accepts inbound connections from node 2 as persistent with
    // explicit peering on. persistent_peers_only stays false so
    // remove_persistent_peer does not tear down an inbound connection.
    let mut config1 = make_config(base_port);
    config1.gossipsub.enable_explicit_peering = true;

    // Node 2 bootstraps from node 1, so node 1 sees an inbound connection.
    let mut config2 = make_config(base_port + 1);
    config2.gossipsub.enable_explicit_peering = true;
    config2.persistent_peers = vec![node1_addr_with_p2p.clone()];

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
        config2,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2".to_string()),
    )
    .await
    .unwrap();

    let node2_libp2p_peer_id = handle2.peer_id().to_libp2p();
    let node2_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port + 1,
        node2_libp2p_peer_id
    )
    .parse()
    .unwrap();

    // Wait for node 2 to dial in.
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
    assert!(connected, "Inbound peer should connect");

    // Now add node 2 as a persistent peer on node 1 (already connected
    // inbound). The add path must mark it explicit immediately.
    let result = handle1
        .add_persistent_peer(node2_addr_with_p2p.clone())
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    sleep(Duration::from_millis(200)).await;

    let dump = handle1.dump_state().await.unwrap();
    assert!(
        dump.peers
            .get(&node2_libp2p_peer_id)
            .expect("node-2 should be in peer_info")
            .is_explicit,
        "runtime-added persistent peer should enter gossipsub explicit set"
    );

    // Remove the persistent peer. Connection must remain (inbound +
    // !persistent_peers_only), so peer_info stays; is_explicit must flip
    // to false via the remove path itself.
    let result = handle1
        .remove_persistent_peer(node2_addr_with_p2p)
        .await
        .unwrap();
    assert_eq!(result, Ok(()));

    sleep(Duration::from_millis(200)).await;

    let dump = handle1.dump_state().await.unwrap();
    let peer_info = dump
        .peers
        .get(&node2_libp2p_peer_id)
        .expect("peer should still be connected after remove");
    assert!(
        !peer_info.is_explicit,
        "remove_persistent_peer must clear the gossipsub explicit flag"
    );
    assert!(
        !peer_info.peer_type.is_persistent(),
        "remove_persistent_peer must clear the persistent classification"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// With explicit peering disabled (the default), runtime add of a
/// persistent peer must not mark the peer as explicit.
#[tokio::test]
async fn test_runtime_add_no_op_when_explicit_peering_disabled() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 38000;

    // Default config: gossipsub.enable_explicit_peering is false.
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

    let node2_libp2p_peer_id = handle2.peer_id().to_libp2p();
    let node2_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port + 1,
        node2_libp2p_peer_id
    )
    .parse()
    .unwrap();

    handle1
        .add_persistent_peer(node2_addr_with_p2p.clone())
        .await
        .unwrap()
        .unwrap();

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

    sleep(Duration::from_millis(200)).await;

    let dump = handle1.dump_state().await.unwrap();
    let peer_info = dump
        .peers
        .get(&node2_libp2p_peer_id)
        .expect("node-2 should be in peer_info");
    assert!(
        !peer_info.is_explicit,
        "explicit peering disabled: peer must not be marked explicit"
    );
    assert!(
        peer_info.peer_type.is_persistent(),
        "peer should still be classified as persistent"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// A peer-only address (/p2p/<peer_id>, no transport) in `persistent_peers` with
/// `persistent_peers_only` enabled must accept inbound connections from that peer.
#[tokio::test]
async fn peer_only_addr_accepts_inbound_in_persistent_peers_only_mode() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let base_port = 39000;

    // Derive node2's peer_id from its keypair before spawning either node.
    let node2_libp2p_peer_id = keypair2.public().to_peer_id();
    let node2_peer_only_addr: malachitebft_network::Multiaddr =
        format!("/p2p/{}", node2_libp2p_peer_id).parse().unwrap();

    // Node1: accepts inbound connections only from configured persistent peers;
    // knows node2 by peer_id alone (no transport address).
    let mut config1 = make_config(base_port);
    config1.persistent_peers_only = true;
    config1.discovery.persistent_peers_only = true;
    config1.persistent_peers = vec![node2_peer_only_addr];

    // Node2: dials node1 via its full transport address.
    let node1_libp2p_peer_id = keypair1.public().to_peer_id();
    let node1_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port, node1_libp2p_peer_id
    )
    .parse()
    .unwrap();
    let mut config2 = make_config(base_port + 1);
    config2.persistent_peers = vec![node1_addr_with_p2p];

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1-poa".to_string()),
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
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2-poa".to_string()),
    )
    .await
    .unwrap();

    // Wait for node1 to receive the inbound connection from node2.
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
    assert!(connected, "Node2 should connect to node1 inbound");

    // Give the Identify handler time to finalize peer classification.
    sleep(Duration::from_millis(200)).await;

    let dump = handle1.dump_state().await.unwrap();
    let peer_info = dump
        .peers
        .get(&node2_libp2p_peer_id)
        .expect("node2 should appear in node1 peer_info");
    assert!(
        peer_info.peer_type.is_persistent(),
        "inbound peer matched by peer-only addr should be classified as persistent"
    );

    handle1.shutdown().await.unwrap();
    handle2.shutdown().await.unwrap();
}

/// A peer-only address (/p2p/<peer_id>, no transport) in `persistent_peers` with
/// `persistent_peers_only` enabled must reject connections from unknown peers.
#[tokio::test]
async fn peer_only_addr_rejects_unknown_in_persistent_peers_only_mode() {
    init_logging();

    let keypair1 = Keypair::generate_ed25519();
    let keypair2 = Keypair::generate_ed25519();
    let keypair3 = Keypair::generate_ed25519();
    let base_port = 39002;

    let node2_libp2p_peer_id = keypair2.public().to_peer_id();
    let node2_peer_only_addr: malachitebft_network::Multiaddr =
        format!("/p2p/{}", node2_libp2p_peer_id).parse().unwrap();

    // Node1: knows only node2 by peer_id; rejects all other inbound peers.
    let node1_libp2p_peer_id = keypair1.public().to_peer_id();
    let node1_addr_with_p2p: malachitebft_network::Multiaddr = format!(
        "/ip4/127.0.0.1/udp/{}/quic-v1/p2p/{}",
        base_port, node1_libp2p_peer_id
    )
    .parse()
    .unwrap();
    let mut config1 = make_config(base_port);
    config1.persistent_peers_only = true;
    config1.discovery.persistent_peers_only = true;
    config1.persistent_peers = vec![node2_peer_only_addr];

    // Node2: dials node1 — must be accepted.
    let mut config2 = make_config(base_port + 1);
    config2.persistent_peers = vec![node1_addr_with_p2p.clone()];

    // Node3: also dials node1 — must be rejected.
    let mut config3 = make_config(base_port + 2);
    config3.persistent_peers = vec![node1_addr_with_p2p];

    let mut handle1 = spawn(
        NetworkIdentity::new(
            "node-1".to_string(),
            keypair1,
            Some("test-address-1".to_string()),
        ),
        config1,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-1-poruj".to_string()),
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
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-2-poruj".to_string()),
    )
    .await
    .unwrap();

    let handle3 = spawn(
        NetworkIdentity::new(
            "node-3".to_string(),
            keypair3,
            Some("test-address-3".to_string()),
        ),
        config3,
        malachitebft_metrics::SharedRegistry::global().with_moniker("node-3-poruj".to_string()),
    )
    .await
    .unwrap();

    let node3_peer_id = handle3.peer_id();

    // Node2 should connect and be accepted; node3 should connect briefly then
    // be disconnected by the persistent_peers_only filter.
    let mut node2_connected = false;
    let mut node3_rejected = false;
    for _ in 0..100 {
        tokio::select! {
            event = handle1.recv() => {
                match event {
                    Some(Event::PeerConnected(peer_id)) if peer_id == handle2.peer_id() => {
                        node2_connected = true;
                    }
                    Some(Event::PeerDisconnected(peer_id)) if peer_id == node3_peer_id => {
                        node3_rejected = true;
                    }
                    _ => {}
                }
                if node2_connected && node3_rejected {
                    break;
                }
            }
            _ = sleep(Duration::from_millis(100)) => {}
        }
    }

    assert!(
        node2_connected,
        "Node2 (whitelisted by peer_id) should connect"
    );
    assert!(node3_rejected, "Node3 (unknown peer) should be rejected");

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
