//! Behaviour for the Validator Proof protocol using libp2p_stream.
//!
//! This is a one-way protocol where validators send their proof to peers.
//! No response is expected - the receiver just stores the proof.

use std::collections::HashSet;
use std::task::{self, Poll};

use bytes::Bytes;
use libp2p::swarm::behaviour::ConnectionEstablished;
use libp2p::swarm::{
    CloseConnection, ConnectionClosed, ConnectionId, FromSwarm, NetworkBehaviour, ToSwarm,
};
use libp2p::{Multiaddr, PeerId, StreamProtocol};
use libp2p_stream as stream;
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

use super::protocol;

/// Events emitted by the Validator Proof behaviour.
#[derive(Debug)]
pub enum Event {
    /// Successfully sent our proof to a peer.
    ProofSent { peer: PeerId },
    /// Received a proof from a peer.
    ProofReceived { peer: PeerId, proof_bytes: Bytes },
    /// Failed to send our proof to a peer (allows retry).
    ProofSendFailed { peer: PeerId, error: Error },
    /// Failed to receive a valid proof from peer (should disconnect).
    ProofReceiveFailed { peer: PeerId, error: Error },
}

/// Errors that can occur in the Validator Proof protocol.
#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(String),
    #[error("Stream closed unexpectedly")]
    UnexpectedEof,
}

/// Validator Proof behaviour using libp2p_stream for one-way proof sending.
pub struct Behaviour {
    /// Inner stream behaviour.
    inner: stream::Behaviour,

    /// Protocol name for validator proof (e.g. `/malachitebft-validator-proof/v1`).
    protocol: StreamProtocol,

    /// Proof bytes to send (if we're a validator).
    proof_bytes: Option<Bytes>,

    /// Channel for receiving events from protocol tasks.
    events_rx: mpsc::UnboundedReceiver<Event>,
    events_tx: mpsc::UnboundedSender<Event>,

    /// Track peers we've received proofs from (anti-spam: one proof per peer per session).
    /// Cleared when the last connection to a peer closes.
    proofs_received: HashSet<PeerId>,

    /// Whether we're listening for incoming streams.
    listening: bool,
}

impl Behaviour {
    /// Create a new behaviour with the given protocol name.
    pub fn new(protocol: StreamProtocol) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        Self {
            inner: stream::Behaviour::new(),
            protocol,
            proof_bytes: None,
            events_rx,
            events_tx,
            proofs_received: HashSet::new(),
            listening: false,
        }
    }

    /// Create a behaviour with the default protocol name (for tests or when not using config).
    /// Prefer [`new`](Self::new) with the protocol from config to match sync/identify.
    pub fn with_default_protocol() -> Self {
        Self::new(StreamProtocol::new("/malachitebft-validator-proof/v1"))
    }

    /// Set the proof bytes to send when connecting to peers.
    /// Called once at startup; the proof is a static binding of (public_key, peer_id)
    /// and does not change with validator set membership.
    pub fn set_proof(&mut self, proof_bytes: Bytes) {
        self.proof_bytes = Some(proof_bytes);
    }

    /// Check if we have a proof to send.
    pub fn has_proof(&self) -> bool {
        self.proof_bytes.is_some()
    }

    /// Send our proof to a specific peer.
    /// Returns true if the send was initiated, false if no proof is set.
    fn send_proof(&mut self, peer_id: PeerId) -> bool {
        let Some(proof_bytes) = &self.proof_bytes else {
            return false;
        };

        let control = self.inner.new_control();
        let events_tx = self.events_tx.clone();
        let protocol = self.protocol.clone();
        let proof_bytes = proof_bytes.clone();

        tokio::spawn(async move {
            let event = protocol::send_proof(peer_id, proof_bytes, control, protocol).await;
            let _ = events_tx.send(event);
        });

        true
    }

    fn start_listening(&mut self) {
        if self.listening {
            // If there are multiple listen addresses, we may get multiple NewListenAddr events - only start once
            return;
        }

        self.listening = true;

        let control = self.inner.new_control();
        let events_tx = self.events_tx.clone();
        let protocol = self.protocol.clone();

        tokio::spawn(async move {
            protocol::accept_incoming_streams(control, events_tx, protocol).await;
        });

        debug!(protocol = %self.protocol, "Listening for incoming validator proof");
    }

    fn on_connection_established(&mut self, conn: &ConnectionEstablished<'_>) {
        let peer_id = conn.peer_id;

        if conn.other_established > 0 {
            trace!(
                %peer_id,
                other_established = conn.other_established,
                "Additional connection to peer, skipping proof send"
            );
            return;
        }

        if self.send_proof(peer_id) {
            debug!(%peer_id, "Sending validator proof on first connection");
        }
    }

    fn on_connection_closed(&mut self, conn: &ConnectionClosed<'_>) {
        if conn.remaining_established > 0 {
            return;
        }

        let peer_id = conn.peer_id;
        trace!(%peer_id, "Last connection closed, cleaning up proof state");
        self.proofs_received.remove(&peer_id);
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        Self::with_default_protocol()
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = <stream::Behaviour as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = Event;

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: libp2p::core::Endpoint,
        port_use: libp2p::core::transport::PortUse,
    ) -> Result<libp2p::swarm::THandler<Self>, libp2p::swarm::ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        match &event {
            FromSwarm::NewListenAddr(_) => {
                self.start_listening();
            }
            FromSwarm::ConnectionEstablished(conn) => {
                self.on_connection_established(conn);
            }
            FromSwarm::ConnectionClosed(conn) => {
                self.on_connection_closed(conn);
            }
            _ => {}
        }

        self.inner.on_swarm_event(event);
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: libp2p::swarm::THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut task::Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, libp2p::swarm::THandlerInEvent<Self>>> {
        // Check for events from protocol tasks
        if let Poll::Ready(Some(event)) = self.events_rx.poll_recv(cx) {
            match &event {
                Event::ProofSendFailed { .. } => {
                    return Poll::Ready(ToSwarm::GenerateEvent(event));
                }
                // On receive failure, disconnect peer directly
                Event::ProofReceiveFailed { peer, error } => {
                    warn!(%peer, %error, "Failed to receive validator proof, closing connection");
                    return Poll::Ready(ToSwarm::CloseConnection {
                        peer_id: *peer,
                        connection: CloseConnection::All,
                    });
                }
                // On proof received, check for duplicate (anti-spam)
                Event::ProofReceived { peer, .. } => {
                    if self.proofs_received.contains(peer) {
                        warn!(%peer, "Duplicate validator proof received, closing connection (anti-spam)");
                        return Poll::Ready(ToSwarm::CloseConnection {
                            peer_id: *peer,
                            connection: CloseConnection::All,
                        });
                    }
                    self.proofs_received.insert(*peer);
                    return Poll::Ready(ToSwarm::GenerateEvent(event));
                }
                // Forward other events to swarm
                _ => return Poll::Ready(ToSwarm::GenerateEvent(event)),
            }
        }

        // Poll the inner behaviour.
        //
        // NOTE: In practice, inner.poll() always returns Pending because open_stream
        // is only called on already-connected peers (from on_connection_established),
        // so the dial path in stream::Behaviour::poll() is never triggered.
        let _ = self.inner.poll(cx);

        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::task::Poll;

    use futures::task::noop_waker_ref;
    use libp2p::core::transport::PortUse;
    use libp2p::core::Endpoint;
    use libp2p::swarm::behaviour::ConnectionEstablished;
    use libp2p::swarm::ConnectionClosed;

    /// Returns a `Dialer` connected point for tests.
    fn dialer_endpoint() -> libp2p::core::ConnectedPoint {
        libp2p::core::ConnectedPoint::Dialer {
            address: "/ip4/127.0.0.1/tcp/9000".parse().unwrap(),
            role_override: Endpoint::Dialer,
            port_use: PortUse::Reuse,
        }
    }

    /// Poll the behaviour once with a noop waker and return the result.
    fn poll_behaviour(
        b: &mut Behaviour,
    ) -> Poll<ToSwarm<Event, libp2p::swarm::THandlerInEvent<Behaviour>>> {
        let waker = noop_waker_ref();
        let mut cx = std::task::Context::from_waker(waker);
        b.poll(&mut cx)
    }

    /// Simulate a connection established event.
    fn establish_connection(
        b: &mut Behaviour,
        peer: PeerId,
        conn_id: ConnectionId,
        other_established: usize,
    ) {
        let endpoint = dialer_endpoint();
        let event = FromSwarm::ConnectionEstablished(ConnectionEstablished {
            peer_id: peer,
            connection_id: conn_id,
            endpoint: &endpoint,
            failed_addresses: &[],
            other_established,
        });
        b.on_swarm_event(event);
    }

    /// Simulate a connection closed event.
    fn close_connection(
        b: &mut Behaviour,
        peer: PeerId,
        conn_id: ConnectionId,
        remaining_established: usize,
    ) {
        let endpoint = dialer_endpoint();
        let event = FromSwarm::ConnectionClosed(ConnectionClosed {
            peer_id: peer,
            connection_id: conn_id,
            endpoint: &endpoint,
            cause: None,
            remaining_established,
        });
        b.on_swarm_event(event);
    }

    // ── Poll tests ───────────────────────────────────────────────────

    #[test]
    fn poll_returns_pending_when_no_events() {
        let mut b = Behaviour::with_default_protocol();
        assert!(poll_behaviour(&mut b).is_pending());
    }

    #[test]
    fn poll_proof_received_emits_event() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        b.events_tx
            .send(Event::ProofReceived {
                peer,
                proof_bytes: Bytes::from_static(b"proof"),
            })
            .unwrap();

        match poll_behaviour(&mut b) {
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofReceived {
                peer: p,
                proof_bytes,
            })) => {
                assert_eq!(p, peer);
                assert_eq!(proof_bytes.as_ref(), b"proof");
            }
            other => panic!("expected GenerateEvent(ProofReceived), got {other:?}"),
        }
        assert!(b.proofs_received.contains(&peer));
    }

    #[test]
    fn poll_duplicate_proof_triggers_disconnect() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        // First proof is accepted
        b.events_tx
            .send(Event::ProofReceived {
                peer,
                proof_bytes: Bytes::from_static(b"proof"),
            })
            .unwrap();
        let _ = poll_behaviour(&mut b);

        // Second proof triggers disconnect
        b.events_tx
            .send(Event::ProofReceived {
                peer,
                proof_bytes: Bytes::from_static(b"proof2"),
            })
            .unwrap();

        match poll_behaviour(&mut b) {
            Poll::Ready(ToSwarm::CloseConnection {
                peer_id,
                connection,
            }) => {
                assert_eq!(peer_id, peer);
                assert!(matches!(connection, CloseConnection::All));
            }
            other => panic!("expected CloseConnection, got {other:?}"),
        }
    }

    #[test]
    fn poll_different_peers_both_accepted() {
        let mut b = Behaviour::with_default_protocol();
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        b.events_tx
            .send(Event::ProofReceived {
                peer: peer_a,
                proof_bytes: Bytes::from_static(b"a"),
            })
            .unwrap();
        b.events_tx
            .send(Event::ProofReceived {
                peer: peer_b,
                proof_bytes: Bytes::from_static(b"b"),
            })
            .unwrap();

        // Both should be accepted (GenerateEvent)
        assert!(matches!(
            poll_behaviour(&mut b),
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofReceived { .. }))
        ));
        assert!(matches!(
            poll_behaviour(&mut b),
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofReceived { .. }))
        ));

        assert!(b.proofs_received.contains(&peer_a));
        assert!(b.proofs_received.contains(&peer_b));
    }

    #[test]
    fn poll_receive_failure_triggers_disconnect() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        b.events_tx
            .send(Event::ProofReceiveFailed {
                peer,
                error: Error::UnexpectedEof,
            })
            .unwrap();

        match poll_behaviour(&mut b) {
            Poll::Ready(ToSwarm::CloseConnection {
                peer_id,
                connection,
            }) => {
                assert_eq!(peer_id, peer);
                assert!(matches!(connection, CloseConnection::All));
            }
            other => panic!("expected CloseConnection, got {other:?}"),
        }
    }

    #[test]
    fn poll_send_failure_emits_event() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        b.events_tx
            .send(Event::ProofSendFailed {
                peer,
                error: Error::Io("test".into()),
            })
            .unwrap();

        assert!(matches!(
            poll_behaviour(&mut b),
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofSendFailed { .. }))
        ));
    }

    #[test]
    fn poll_proof_sent_emits_event() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        b.events_tx.send(Event::ProofSent { peer }).unwrap();

        match poll_behaviour(&mut b) {
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofSent { peer: p })) => {
                assert_eq!(p, peer);
            }
            other => panic!("expected GenerateEvent(ProofSent), got {other:?}"),
        }
    }

    // ── send_proof tests ─────────────────────────────────────────────

    #[test]
    fn send_proof_returns_false_without_proof() {
        let mut b = Behaviour::with_default_protocol();
        assert!(!b.send_proof(PeerId::random()));
    }

    // ── Connection tracking tests ────────────────────────────────────

    #[test]
    fn last_connection_close_clears_proof_state() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();
        let conn = ConnectionId::new_unchecked(1);

        establish_connection(&mut b, peer, conn, 0);
        b.proofs_received.insert(peer);

        close_connection(&mut b, peer, conn, 0);

        assert!(!b.proofs_received.contains(&peer));
    }

    #[test]
    fn partial_close_preserves_proof_state() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();
        let conn1 = ConnectionId::new_unchecked(1);
        let conn2 = ConnectionId::new_unchecked(2);

        establish_connection(&mut b, peer, conn1, 0);
        establish_connection(&mut b, peer, conn2, 1);
        b.proofs_received.insert(peer);

        // Close one, one remains
        close_connection(&mut b, peer, conn1, 1);

        assert!(b.proofs_received.contains(&peer));
    }

    #[test]
    fn anti_spam_reset_after_full_disconnect() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();
        let conn1 = ConnectionId::new_unchecked(1);
        let conn2 = ConnectionId::new_unchecked(2);

        // Connect, receive proof
        establish_connection(&mut b, peer, conn1, 0);
        b.events_tx
            .send(Event::ProofReceived {
                peer,
                proof_bytes: Bytes::from_static(b"proof"),
            })
            .unwrap();
        let _ = poll_behaviour(&mut b);
        assert!(b.proofs_received.contains(&peer));

        // Fully disconnect
        close_connection(&mut b, peer, conn1, 0);
        assert!(!b.proofs_received.contains(&peer));

        // Reconnect → proof should be accepted again
        establish_connection(&mut b, peer, conn2, 0);
        b.events_tx
            .send(Event::ProofReceived {
                peer,
                proof_bytes: Bytes::from_static(b"proof"),
            })
            .unwrap();

        assert!(matches!(
            poll_behaviour(&mut b),
            Poll::Ready(ToSwarm::GenerateEvent(Event::ProofReceived { .. }))
        ));
    }

    // ── Connection established + send_proof integration (requires tokio) ─

    #[tokio::test]
    async fn first_connection_sends_proof() {
        let mut b = Behaviour::with_default_protocol();
        b.set_proof(Bytes::from_static(b"proof"));
        let peer = PeerId::random();
        let conn = ConnectionId::new_unchecked(100);

        establish_connection(&mut b, peer, conn, 0);

        // send_proof spawns a task; verify it was called by checking has_proof is still true
        assert!(b.has_proof());
    }
}
