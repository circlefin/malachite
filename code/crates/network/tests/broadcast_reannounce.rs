//! Real-swarm test that a second `subscribe` on the same topic is delivered.
//!
//! Scatter only announces on first connection. The network task re-calls
//! `subscribe` when another connection to that peer opens, and when one
//! connection closes while another remains; this locks the crate contract
//! that call relies on.

use std::time::Duration;

use arc_malachitebft_network::{Channel, ChannelNames};
use libp2p::futures::StreamExt;
use libp2p::swarm::SwarmEvent;
use libp2p::{noise, tcp, yamux, Multiaddr, Swarm, SwarmBuilder};
use libp2p_broadcast as broadcast;

const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

fn build_swarm() -> Swarm<broadcast::Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("tcp transport")
        .with_behaviour(|_| broadcast::Behaviour::new(broadcast::Config::default()))
        .expect("behaviour")
        .with_swarm_config(|c| c.with_idle_connection_timeout(IDLE_TIMEOUT))
        .build()
}

async fn wait_listen_addr<B>(swarm: &mut Swarm<B>) -> Multiaddr
where
    B: libp2p::swarm::NetworkBehaviour,
{
    loop {
        if let SwarmEvent::NewListenAddr { address, .. } = swarm.select_next_some().await {
            return address;
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn second_subscribe_is_delivered_to_an_already_connected_peer() {
    let names = ChannelNames::default();
    let topic = Channel::Sync.to_broadcast_topic(&names);

    let mut announcer = build_swarm();
    let announcer_id = *announcer.local_peer_id();
    announcer.behaviour_mut().subscribe(topic);

    let mut listener = build_swarm();
    listener
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let addr = wait_listen_addr(&mut listener).await;
    announcer.dial(addr).unwrap();

    let first = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                _ = announcer.select_next_some() => {}
                event = listener.select_next_some() => {
                    if let SwarmEvent::Behaviour(broadcast::Event::Subscribed(peer, t)) = event {
                        if peer == announcer_id && t == topic {
                            break;
                        }
                    }
                }
            }
        }
    })
    .await;
    assert!(first.is_ok(), "first subscribe was not delivered");

    announcer.behaviour_mut().subscribe(topic);

    let second = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                _ = announcer.select_next_some() => {}
                event = listener.select_next_some() => {
                    if let SwarmEvent::Behaviour(broadcast::Event::Subscribed(peer, t)) = event {
                        if peer == announcer_id && t == topic {
                            break;
                        }
                    }
                }
            }
        }
    })
    .await;
    assert!(
        second.is_ok(),
        "re-subscribe on a live connection was not delivered"
    );
}
