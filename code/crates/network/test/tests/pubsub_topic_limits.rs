use std::collections::HashSet;
use std::time::Duration;

use malachitebft_config::TransportProtocol;
use malachitebft_metrics::{export, SharedRegistry};
use malachitebft_network::handle::{CtrlHandle, RecvHandle};
use malachitebft_network::{
    spawn, Channel, ChannelNames, Config, DiscoveryConfig, Event, GossipSubConfig, Keypair,
    NetworkIdentity, PeerId, ProtocolNames, PubSubMaxSizePerTopic, PubSubProtocol,
};
use rand::Rng;
use tokio::time::{sleep, timeout};

fn make_config(port: u16, persistent_peers: Vec<u16>) -> Config {
    Config {
        listen_addr: TransportProtocol::Quic.multiaddr("127.0.0.1", port as usize),
        persistent_peers: persistent_peers
            .into_iter()
            .map(|peer_port| TransportProtocol::Quic.multiaddr("127.0.0.1", peer_port as usize))
            .collect(),
        persistent_peers_only: false,
        discovery: DiscoveryConfig {
            enabled: false,
            max_connections_per_ip: usize::MAX,
            ..Default::default()
        },
        idle_connection_timeout: Duration::from_secs(60),
        transport: malachitebft_network::TransportProtocol::Quic,
        gossipsub: GossipSubConfig::default(),
        pubsub_protocol: PubSubProtocol::GossipSub,
        channel_names: ChannelNames::default(),
        rpc_max_size: 10 * 1024 * 1024,
        pubsub_max_size: 4 * 1024 * 1024,
        pubsub_max_size_per_topic: PubSubMaxSizePerTopic::default(),
        sync_request_timeout: Duration::from_secs(10),
        sync_max_request_size: 1024 * 1024,
        sync_parallel_requests: 5,
        enable_consensus: true,
        enable_sync: false,
        protocol_names: ProtocolNames::default(),
    }
}

async fn wait_for_peers(handle: &mut RecvHandle, expected: &[PeerId]) {
    let mut remaining = expected.iter().copied().collect::<HashSet<_>>();

    timeout(Duration::from_secs(10), async {
        while !remaining.is_empty() {
            match handle.recv().await {
                Some(Event::PeerConnected(peer_id)) => {
                    remaining.remove(&peer_id);
                }
                Some(_) => {}
                None => panic!("network stopped before all peers connected"),
            }
        }
    })
    .await
    .expect("timed out waiting for peers to connect");
}

async fn receive_message(handle: &mut RecvHandle, channel: Channel, expected: &[u8]) {
    timeout(Duration::from_secs(5), async {
        loop {
            match handle.recv().await {
                Some(Event::ConsensusMessage(received_channel, _, _, data))
                    if received_channel == channel =>
                {
                    assert_eq!(data.as_ref(), expected);
                    return;
                }
                Some(_) => {}
                None => panic!("network stopped before receiving the message"),
            }
        }
    })
    .await
    .expect("timed out waiting for a pubsub message");
}

async fn assert_no_message(handle: &mut RecvHandle, channel: Channel) {
    let received = timeout(Duration::from_secs(2), async {
        loop {
            match handle.recv().await {
                Some(Event::ConsensusMessage(received_channel, _, _, data))
                    if received_channel == channel =>
                {
                    return Some(data.len());
                }
                Some(_) => {}
                None => return None,
            }
        }
    })
    .await;

    match received {
        Err(_) => {}
        Ok(Some(size)) => panic!("received an oversized {channel} message of {size} bytes"),
        Ok(None) => panic!("network stopped while checking for an oversized message"),
    }
}

async fn shutdown(handles: [CtrlHandle; 3]) {
    let [sender, relay, observer] = handles;
    let (sender_result, relay_result, observer_result) = tokio::join!(
        sender.wait_shutdown(),
        relay.wait_shutdown(),
        observer.wait_shutdown()
    );
    sender_result.unwrap();
    relay_result.unwrap();
    observer_result.unwrap();
}

#[tokio::test]
async fn proposal_parts_limit_is_enforced_before_delivery_and_relay() {
    let base_port: u16 = rand::thread_rng().gen_range(30_000..50_000);
    let relay_port = base_port;
    let sender_port = base_port + 1;
    let observer_port = base_port + 2;

    let relay_keypair = Keypair::generate_ed25519();
    let sender_keypair = Keypair::generate_ed25519();
    let observer_keypair = Keypair::generate_ed25519();
    let allowed_message = vec![0x2a; 1024];
    let oversized_message = vec![0x2a; allowed_message.len() + 1];
    let proposal_parts_max_payload_size = allowed_message.len();

    let relay_moniker = format!("topic-limit-relay-{base_port}");
    let mut relay_config = make_config(relay_port, vec![]);
    relay_config.gossipsub.enable_peer_scoring = true;
    relay_config.pubsub_max_size_per_topic.proposal_parts = Some(proposal_parts_max_payload_size);

    let relay_handle = spawn(
        NetworkIdentity::new(relay_moniker.clone(), relay_keypair, None),
        relay_config,
        SharedRegistry::global().with_moniker(relay_moniker.clone()),
    )
    .await
    .unwrap();

    sleep(Duration::from_millis(200)).await;

    let observer_handle = spawn(
        NetworkIdentity::new(
            format!("topic-limit-observer-{base_port}"),
            observer_keypair,
            None,
        ),
        make_config(observer_port, vec![relay_port]),
        SharedRegistry::global().with_moniker(format!("topic-limit-observer-{base_port}")),
    )
    .await
    .unwrap();

    let sender_handle = spawn(
        NetworkIdentity::new(
            format!("topic-limit-sender-{base_port}"),
            sender_keypair,
            None,
        ),
        make_config(sender_port, vec![relay_port]),
        SharedRegistry::global().with_moniker(format!("topic-limit-sender-{base_port}")),
    )
    .await
    .unwrap();

    let relay_peer_id = relay_handle.peer_id();
    let observer_peer_id = observer_handle.peer_id();
    let sender_peer_id = sender_handle.peer_id();
    let (mut relay_recv, relay_ctrl) = relay_handle.split();
    let (mut observer_recv, observer_ctrl) = observer_handle.split();
    let (mut sender_recv, sender_ctrl) = sender_handle.split();

    wait_for_peers(&mut relay_recv, &[sender_peer_id, observer_peer_id]).await;
    wait_for_peers(&mut sender_recv, &[relay_peer_id]).await;
    wait_for_peers(&mut observer_recv, &[relay_peer_id]).await;
    sleep(Duration::from_secs(2)).await;

    sender_ctrl
        .publish(Channel::ProposalParts, allowed_message.clone().into())
        .await
        .unwrap();
    receive_message(&mut relay_recv, Channel::ProposalParts, &allowed_message).await;
    receive_message(&mut observer_recv, Channel::ProposalParts, &allowed_message).await;

    sender_ctrl
        .publish(Channel::ProposalParts, oversized_message.clone().into())
        .await
        .unwrap();
    assert_no_message(&mut relay_recv, Channel::ProposalParts).await;
    assert_no_message(&mut observer_recv, Channel::ProposalParts).await;

    let mut metrics = String::new();
    export(&mut metrics);
    let invalid_metric = metrics
        .lines()
        .find(|line| {
            line.starts_with("malachitebft_network_gossipsub_invalid_messages_per_topic_total")
                && line.contains(&format!("moniker=\"{relay_moniker}\""))
                && line.contains("/proposal_parts")
        })
        .expect("missing invalid-message metric for the proposal-parts topic");
    let invalid_count = invalid_metric
        .split_whitespace()
        .last()
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(invalid_count >= 1);

    sender_ctrl
        .publish(Channel::Consensus, oversized_message.clone().into())
        .await
        .unwrap();
    receive_message(&mut relay_recv, Channel::Consensus, &oversized_message).await;
    receive_message(&mut observer_recv, Channel::Consensus, &oversized_message).await;

    shutdown([sender_ctrl, relay_ctrl, observer_ctrl]).await;
}
