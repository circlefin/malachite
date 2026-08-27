//! Real-swarm integration tests for the validator-proof protocol.
//!
//! These build actual libp2p swarms over TCP + noise + yamux, so they exercise
//! the true request/response admission path rather than mocked events.

use std::collections::HashSet;
use std::io;
use std::time::Duration;

use arc_malachitebft_network::validator_proof::{Behaviour, Event};
use async_trait::async_trait;
use asynchronous_codec::{FramedRead, FramedWrite};
use bytes::Bytes;
use libp2p::futures::channel::oneshot;
use libp2p::futures::io::{AsyncRead, AsyncWrite};
use libp2p::futures::{SinkExt, StreamExt};
use libp2p::request_response::{self, ProtocolSupport};
use libp2p::swarm::{ConnectionError, SwarmEvent};
use libp2p::{noise, tcp, yamux, Multiaddr, Stream, StreamProtocol, Swarm, SwarmBuilder};
use unsigned_varint::codec::UviBytes;

const PROTOCOL: &str = "/malachitebft-validator-proof/v1";

/// Kept well above every test's assertion window so a connection never closes
/// from an idle timeout during a test — a disconnect is always a deliberate one.
const IDLE_TIMEOUT: Duration = Duration::from_secs(60);

fn proto() -> StreamProtocol {
    StreamProtocol::new(PROTOCOL)
}

fn build_swarm() -> Swarm<Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("tcp transport")
        .with_behaviour(|_| Behaviour::with_default_protocol())
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
async fn proof_delivered_once_over_real_swarm() {
    let mut sender = build_swarm();
    let proof = Bytes::from_static(b"validator-proof-bytes");
    sender.behaviour_mut().set_proof(proof.clone());
    let sender_id = *sender.local_peer_id();

    let mut receiver = build_swarm();
    receiver
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let addr = wait_listen_addr(&mut receiver).await;

    sender.dial(addr).unwrap();

    let (peer, proof_bytes) = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                _ = sender.select_next_some() => {}
                event = receiver.select_next_some() => {
                    if let SwarmEvent::Behaviour(Event::ProofReceived { peer, proof_bytes }) = event {
                        break (peer, proof_bytes);
                    }
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the proof to be received");

    assert_eq!(peer, sender_id);
    assert_eq!(proof_bytes, proof);
}

/// Many senders open proof streams at once against a single receiver.
/// Per-connection admission must deliver every proof, with none dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_senders_all_delivered() {
    const SENDERS: usize = 8;

    let mut receiver = build_swarm();
    receiver
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let addr = wait_listen_addr(&mut receiver).await;

    let mut expected = HashSet::new();
    for i in 0..SENDERS {
        let mut sender = build_swarm();
        sender
            .behaviour_mut()
            .set_proof(Bytes::copy_from_slice(format!("proof-{i}").as_bytes()));
        expected.insert(*sender.local_peer_id());
        sender.dial(addr.clone()).unwrap();

        // Drive each sender independently so all proofs are in flight together.
        tokio::spawn(async move {
            loop {
                let _ = sender.select_next_some().await;
            }
        });
    }

    let seen = tokio::time::timeout(Duration::from_secs(30), async {
        let mut seen = HashSet::new();
        loop {
            if let SwarmEvent::Behaviour(Event::ProofReceived { peer, .. }) =
                receiver.select_next_some().await
            {
                seen.insert(peer);
                if seen.len() == SENDERS {
                    break seen;
                }
            }
        }
    })
    .await
    .expect("timed out before every proof was received");

    assert_eq!(seen, expected);
}

/// A malformed (oversized) request must disconnect the peer, not leave it
/// connected. Uses a raw request_response sender that writes an oversized frame
/// on the same `/v1` protocol.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn malformed_request_disconnects_peer() {
    let mut sender = build_oversized_sender();
    let sender_id = *sender.local_peer_id();

    let mut receiver = build_swarm();
    receiver
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let receiver_id = *receiver.local_peer_id();
    let addr = wait_listen_addr(&mut receiver).await;

    sender.dial(addr).unwrap();

    // Send the oversized request once the connection is up.
    let mut sent = false;
    let closed = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            tokio::select! {
                event = sender.select_next_some() => {
                    if let SwarmEvent::ConnectionEstablished { .. } = event {
                        if !sent {
                            sender
                                .behaviour_mut()
                                .send_request(&receiver_id, Bytes::from(vec![0u8; 2048]));
                            sent = true;
                        }
                    }
                }
                event = receiver.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(Event::ProofReceived { .. }) => {
                            panic!("malformed request must not be accepted as a proof");
                        }
                        SwarmEvent::ConnectionClosed { peer_id, cause, .. } if peer_id == sender_id => {
                            assert!(
                                !matches!(cause, Some(ConnectionError::KeepAliveTimeout)),
                                "connection must close from the malformed-proof disconnect, not an idle timeout"
                            );
                            break true;
                        }
                        _ => {}
                    }
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the malformed peer to be disconnected");

    assert!(closed);
}

fn build_oversized_sender() -> Swarm<request_response::Behaviour<OversizedCodec>> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("tcp transport")
        .with_behaviour(|_| {
            request_response::Behaviour::with_codec(
                OversizedCodec,
                [(proto(), ProtocolSupport::Full)],
                request_response::Config::default(),
            )
        })
        .expect("behaviour")
        .with_swarm_config(|c| c.with_idle_connection_timeout(IDLE_TIMEOUT))
        .build()
}

/// A codec that writes an oversized length-delimited frame on the wire, to
/// simulate a misbehaving/legacy peer sending a malformed proof request.
#[derive(Clone, Default)]
struct OversizedCodec;

fn oversized_framing() -> UviBytes {
    let mut framing = UviBytes::default();
    framing.set_max_len(8192);
    framing
}

#[async_trait]
impl request_response::Codec for OversizedCodec {
    type Protocol = libp2p::StreamProtocol;
    type Request = Bytes;
    type Response = ();

    async fn read_request<T>(&mut self, _: &Self::Protocol, _: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        Ok(Bytes::new())
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        _: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        Ok(())
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let mut framed = FramedWrite::new(io, oversized_framing());
        framed.send(req).await?;
        framed.close().await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        _: &mut T,
        _: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        Ok(())
    }
}

// ── Cross-version compatibility with the earlier libp2p-stream implementation ──
//
// The earlier validator-proof implementation carried the proof over a raw
// `libp2p-stream` substream (open → write UVI frame → close). These tests build
// such a peer to confirm the request bytes interoperate in both directions.

fn build_legacy_swarm() -> Swarm<libp2p_stream::Behaviour> {
    SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .expect("tcp transport")
        .with_behaviour(|_| libp2p_stream::Behaviour::new())
        .expect("behaviour")
        .with_swarm_config(|c| c.with_idle_connection_timeout(IDLE_TIMEOUT))
        .build()
}

fn uvi_framing() -> UviBytes {
    let mut framing = UviBytes::default();
    framing.set_max_len(1024);
    framing
}

async fn write_uvi_frame(stream: Stream, bytes: Bytes) {
    let mut framed = FramedWrite::new(stream, uvi_framing());
    framed.send(bytes).await.expect("write frame");
    framed.close().await.expect("close frame");
}

async fn read_uvi_frame(stream: Stream) -> Bytes {
    let mut framed = FramedRead::new(stream, uvi_framing());
    let item = framed.next().await.expect("a frame").expect("valid frame");
    item.into()
}

/// Old libp2p-stream sender → fixed request_response receiver.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn legacy_stream_sender_to_fixed_receiver() {
    let proof = Bytes::from_static(b"legacy-sender-proof");

    let mut receiver = build_swarm();
    receiver
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let receiver_id = *receiver.local_peer_id();
    let addr = wait_listen_addr(&mut receiver).await;

    let mut sender = build_legacy_swarm();
    let control = sender.behaviour().new_control();
    sender.dial(addr).unwrap();

    // Drive the legacy sender swarm and signal once connected.
    let (connected_tx, connected_rx) = oneshot::channel();
    tokio::spawn(async move {
        let mut connected_tx = Some(connected_tx);
        loop {
            let event = sender.select_next_some().await;
            if matches!(event, SwarmEvent::ConnectionEstablished { .. }) {
                if let Some(tx) = connected_tx.take() {
                    let _ = tx.send(());
                }
            }
        }
    });

    // Once connected, open a raw /v1 stream and write the framed proof.
    let proof_to_send = proof.clone();
    tokio::spawn(async move {
        connected_rx.await.expect("connection established");
        let mut control = control;
        let stream = control
            .open_stream(receiver_id, proto())
            .await
            .expect("open stream");
        write_uvi_frame(stream, proof_to_send).await;
    });

    let received = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let SwarmEvent::Behaviour(Event::ProofReceived { proof_bytes, .. }) =
                receiver.select_next_some().await
            {
                break proof_bytes;
            }
        }
    })
    .await
    .expect("timed out waiting for the legacy proof to be received");

    assert_eq!(received, proof);
}

/// Fixed request_response sender → old libp2p-stream receiver.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fixed_sender_to_legacy_stream_receiver() {
    let proof = Bytes::from_static(b"fixed-sender-proof");

    let mut receiver = build_legacy_swarm();
    let mut incoming = receiver.behaviour().new_control().accept(proto()).unwrap();
    receiver
        .listen_on("/ip4/127.0.0.1/tcp/0".parse().unwrap())
        .unwrap();
    let addr = wait_listen_addr(&mut receiver).await;

    // Drive the legacy receiver swarm so inbound streams are delivered.
    tokio::spawn(async move {
        loop {
            let _ = receiver.select_next_some().await;
        }
    });

    let mut sender = build_swarm();
    sender.behaviour_mut().set_proof(proof.clone());
    sender.dial(addr).unwrap();
    tokio::spawn(async move {
        loop {
            let _ = sender.select_next_some().await;
        }
    });

    let (_, stream) = tokio::time::timeout(Duration::from_secs(20), incoming.next())
        .await
        .expect("timed out waiting for the inbound proof stream")
        .expect("inbound stream");
    let received = tokio::time::timeout(Duration::from_secs(5), read_uvi_frame(stream))
        .await
        .expect("timed out reading the proof frame");

    assert_eq!(received, proof);
}
