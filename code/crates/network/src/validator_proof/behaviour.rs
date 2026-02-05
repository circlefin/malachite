//! Behaviour for the Validator Proof protocol using libp2p_stream.
//!
//! This is a one-way protocol where validators send their proof to peers.
//! No response is expected - the receiver just stores the proof.

use std::collections::{HashMap, HashSet};
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

/// Protocol name for validator proof.
pub const PROTOCOL_NAME: StreamProtocol = StreamProtocol::new("/malachitebft-validator-proof/v1");

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
    #[error("Peer ID mismatch: expected {expected}, got {actual}")]
    PeerIdMismatch { expected: String, actual: String },
    #[error("Invalid peer ID in proof")]
    InvalidPeerId,
    #[error("Stream closed unexpectedly")]
    UnexpectedEof,
}

/// Validator Proof behaviour using libp2p_stream for one-way proof sending.
pub struct Behaviour {
    /// Inner stream behaviour.
    inner: stream::Behaviour,

    /// Proof bytes to send (if we're a validator).
    proof_bytes: Option<Bytes>,

    /// Channel for receiving events from protocol tasks.
    events_rx: mpsc::UnboundedReceiver<Event>,
    events_tx: mpsc::UnboundedSender<Event>,

    /// Track active connections per peer.
    /// Only send proof on first connection, only clean up when all connections close.
    connections: HashMap<PeerId, HashSet<ConnectionId>>,

    /// Track peers we've sent proofs to (to avoid duplicates).
    proofs_sent: HashSet<PeerId>,

    /// Track peers we've received proofs from (anti-spam: one proof per peer per session).
    proofs_received: HashSet<PeerId>,

    /// Whether we're listening for incoming streams.
    listening: bool,
}

impl Behaviour {
    /// Create a new behaviour.
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        Self {
            inner: stream::Behaviour::new(),
            proof_bytes: None,
            events_rx,
            events_tx,
            connections: HashMap::new(),
            proofs_sent: HashSet::new(),
            proofs_received: HashSet::new(),
            listening: false,
        }
    }

    /// Set the proof bytes to send when connecting to peers.
    pub fn set_proof(&mut self, proof_bytes: Bytes) {
        self.proof_bytes = Some(proof_bytes);
    }

    /// Clear the proof (when we're no longer a validator).
    pub fn clear_proof(&mut self) {
        self.proof_bytes = None;
    }

    /// Check if we have a proof to send.
    pub fn has_proof(&self) -> bool {
        self.proof_bytes.is_some()
    }

    /// Send our proof to a specific peer.
    /// Returns true if the send was initiated, false if no proof or already sent.
    pub fn send_proof(&mut self, peer_id: PeerId) -> bool {
        let Some(proof_bytes) = self.proof_bytes.clone() else {
            return false;
        };

        if self.proofs_sent.contains(&peer_id) {
            debug!(%peer_id, "Already sent proof to peer, skipping");
            return false;
        }

        self.proofs_sent.insert(peer_id);

        let control = self.inner.new_control();
        let events_tx = self.events_tx.clone();

        tokio::spawn(async move {
            let event = protocol::send_proof(peer_id, proof_bytes, control).await;
            let _ = events_tx.send(event);
        });

        true
    }

    fn start_listening(&mut self) {
        if self.listening {
            return;
        }
        self.listening = true;

        let control = self.inner.new_control();
        let events_tx = self.events_tx.clone();

        tokio::spawn(protocol::accept_incoming_streams(control, events_tx));
    }

    fn on_connection_established(&mut self, conn: &ConnectionEstablished<'_>) {
        let peer_id = conn.peer_id;

        // Track this connection
        let connections = self.connections.entry(peer_id).or_default();
        let is_first_connection = connections.is_empty();
        connections.insert(conn.connection_id);

        // Only send proof on the first connection to this peer
        if !is_first_connection {
            trace!(
                %peer_id,
                connection_count = connections.len(),
                "Additional connection to peer, skipping proof send"
            );
            return;
        }

        // Send proof if we have one
        if self.send_proof(peer_id) {
            debug!(%peer_id, "Sending validator proof on connection established");
        }
    }

    fn on_connection_closed(&mut self, conn: &ConnectionClosed<'_>) {
        let peer_id = conn.peer_id;

        let Some(connections) = self.connections.get_mut(&peer_id) else {
            return;
        };

        connections.remove(&conn.connection_id);

        // Only clean up when all connections to peer are closed
        if connections.is_empty() {
            trace!(%peer_id, "Last connection closed, cleaning up proof state");
            self.connections.remove(&peer_id);
            self.proofs_sent.remove(&peer_id);
            self.proofs_received.remove(&peer_id);
        }
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        Self::new()
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
                // On send failure, allow retry by removing from sent set
                Event::ProofSendFailed { peer, .. } => {
                    self.proofs_sent.remove(peer);
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

        // Poll the inner behavior
        let _ = self.inner.poll(cx);

        Poll::Pending
    }
}
