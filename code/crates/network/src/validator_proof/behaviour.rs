//! Behaviour for the Validator Proof protocol.
//!
//! A one-way protocol where validators send their proof to peers. It is carried
//! over `request_response`: the request is the proof, the response is empty.
//! `request_response` admits inbound streams per connection, so proofs from
//! different peers do not contend for a single shared inbound slot.

use std::collections::HashSet;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use libp2p::core::transport::PortUse;
use libp2p::core::Endpoint;
use libp2p::request_response::{self, Message, ProtocolSupport};
use libp2p::swarm::{
    CloseConnection, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler,
    THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId, StreamProtocol};
use thiserror::Error;
use tracing::{debug, trace, warn};

use super::codec::{Codec, ProofRequest};

/// Timeout for the request/response handler. Stays above the codec's read
/// timeout so a stalled inbound read becomes an in-band malformed request.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum concurrent proof streams per connection. Normal traffic is one
/// inbound and one outbound proof per connection; the headroom bounds how many
/// streams a single connection can hold open at once.
const MAX_CONCURRENT_STREAMS: usize = 4;

/// Events emitted by the Validator Proof behaviour.
#[derive(Debug)]
pub enum Event {
    /// Our proof was sent to a peer (local send completion, not confirmed delivery).
    ProofSent { peer: PeerId },
    /// Received a proof from a peer.
    ProofReceived { peer: PeerId, proof_bytes: Bytes },
    /// Failed to send our proof to a peer.
    ProofSendFailed { peer: PeerId, error: Error },
}

/// Errors that can occur in the Validator Proof protocol.
#[derive(Clone, Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(String),
}

/// What to do with an inbound proof request.
enum RequestOutcome {
    /// First valid proof from this peer; deliver it.
    Deliver(Bytes),
    /// Reject and disconnect (malformed, or a second proof this session).
    Reject(&'static str),
}

/// Validator Proof behaviour built on `request_response`.
pub struct Behaviour {
    /// Inner request/response behaviour carrying the one-way proof.
    inner: request_response::Behaviour<Codec>,

    /// Proof bytes to send (if we're a validator).
    proof_bytes: Option<Bytes>,

    /// Peers we've received a proof from this session (anti-spam: one proof per
    /// peer per session). Cleared when the last connection to a peer closes.
    proofs_received: HashSet<PeerId>,
}

impl Behaviour {
    /// Create a new behaviour with the given protocol name.
    pub fn new(protocol: StreamProtocol) -> Self {
        let config = request_response::Config::default()
            .with_request_timeout(REQUEST_TIMEOUT)
            .with_max_concurrent_streams(MAX_CONCURRENT_STREAMS);

        let inner = request_response::Behaviour::with_codec(
            Codec,
            [(protocol, ProtocolSupport::Full)],
            config,
        );

        Self {
            inner,
            proof_bytes: None,
            proofs_received: HashSet::new(),
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

    /// Decide what to do with an inbound proof request, recording the peer on
    /// the first valid proof.
    fn classify_request(&mut self, peer: PeerId, request: ProofRequest) -> RequestOutcome {
        match request {
            ProofRequest::Malformed => RequestOutcome::Reject("malformed validator proof"),
            ProofRequest::Proof(bytes) => {
                if self.proofs_received.insert(peer) {
                    RequestOutcome::Deliver(bytes)
                } else {
                    RequestOutcome::Reject("duplicate validator proof")
                }
            }
        }
    }

    fn on_connection_established(&mut self, peer: PeerId) {
        let Some(proof_bytes) = self.proof_bytes.clone() else {
            return;
        };
        debug!(%peer, "Sending validator proof on first connection");
        self.inner
            .send_request(&peer, ProofRequest::Proof(proof_bytes));
    }

    fn on_last_connection_closed(&mut self, peer: PeerId) {
        trace!(%peer, "Last connection closed, cleaning up proof state");
        self.proofs_received.remove(&peer);
    }

    /// Translate an inner request/response event into our behaviour's output,
    /// or `None` if it should be swallowed.
    fn on_inner_event(
        &mut self,
        event: request_response::Event<ProofRequest, ()>,
    ) -> Option<ToSwarm<Event, THandlerInEvent<Self>>> {
        match event {
            request_response::Event::Message {
                peer,
                message:
                    Message::Request {
                        request, channel, ..
                    },
                ..
            } => {
                let outcome = self.classify_request(peer, request);
                // Release the handler; the response carries no bytes.
                let _ = self.inner.send_response(channel, ());
                match outcome {
                    RequestOutcome::Deliver(proof_bytes) => {
                        Some(ToSwarm::GenerateEvent(Event::ProofReceived {
                            peer,
                            proof_bytes,
                        }))
                    }
                    RequestOutcome::Reject(reason) => {
                        warn!(%peer, reason, "Rejecting validator proof, closing connection");
                        Some(ToSwarm::CloseConnection {
                            peer_id: peer,
                            connection: CloseConnection::All,
                        })
                    }
                }
            }
            request_response::Event::Message {
                peer,
                message: Message::Response { .. },
                ..
            } => Some(ToSwarm::GenerateEvent(Event::ProofSent { peer })),
            request_response::Event::OutboundFailure { peer, error, .. } => {
                Some(ToSwarm::GenerateEvent(Event::ProofSendFailed {
                    peer,
                    error: Error::Io(error.to_string()),
                }))
            }
            // Read failures arrive in-band as `ProofRequest::Malformed`; an
            // inbound failure here is a post-delivery stream close.
            request_response::Event::InboundFailure { peer, error, .. } => {
                trace!(%peer, %error, "Inbound validator-proof stream failed");
                None
            }
            request_response::Event::ResponseSent { .. } => None,
        }
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        Self::with_default_protocol()
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler =
        <request_response::Behaviour<Codec> as NetworkBehaviour>::ConnectionHandler;
    type ToSwarm = Event;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.inner
            .handle_pending_inbound_connection(connection_id, local_addr, remote_addr)
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.inner.handle_established_inbound_connection(
            connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        self.inner.handle_pending_outbound_connection(
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.inner.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        // Capture the peer lifecycle transitions before the event is consumed.
        let first_connection = match &event {
            FromSwarm::ConnectionEstablished(conn) if conn.other_established == 0 => {
                Some(conn.peer_id)
            }
            _ => None,
        };
        let last_connection = match &event {
            FromSwarm::ConnectionClosed(conn) if conn.remaining_established == 0 => {
                Some(conn.peer_id)
            }
            _ => None,
        };

        // Register the connection in the inner behaviour before sending, so the
        // send targets the established connection.
        self.inner.on_swarm_event(event);

        if let Some(peer) = first_connection {
            self.on_connection_established(peer);
        }
        if let Some(peer) = last_connection {
            self.on_last_connection_closed(peer);
        }
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        self.inner
            .on_connection_handler_event(peer_id, connection_id, event);
    }

    fn poll(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        // Drain inner events until an actionable one or `Pending`; ignored
        // events (e.g. a post-delivery stream close) continue the loop.
        loop {
            match self.inner.poll(cx) {
                Poll::Ready(ToSwarm::GenerateEvent(event)) => {
                    if let Some(action) = self.on_inner_event(event) {
                        return Poll::Ready(action);
                    }
                }
                Poll::Ready(other) => {
                    return Poll::Ready(other.map_out(|_| unreachable!("handled above")));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_proof_delivers_then_duplicate_rejected() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        assert!(matches!(
            b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof"))),
            RequestOutcome::Deliver(_)
        ));
        assert!(b.proofs_received.contains(&peer));
        assert!(matches!(
            b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof"))),
            RequestOutcome::Reject(_)
        ));
    }

    #[test]
    fn malformed_request_is_rejected() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        assert!(matches!(
            b.classify_request(peer, ProofRequest::Malformed),
            RequestOutcome::Reject(_)
        ));
        // A rejected malformed request does not record the peer.
        assert!(!b.proofs_received.contains(&peer));
    }

    #[test]
    fn different_peers_are_delivered_independently() {
        let mut b = Behaviour::with_default_protocol();
        let peer_a = PeerId::random();
        let peer_b = PeerId::random();

        assert!(matches!(
            b.classify_request(peer_a, ProofRequest::Proof(Bytes::from_static(b"a"))),
            RequestOutcome::Deliver(_)
        ));
        assert!(matches!(
            b.classify_request(peer_b, ProofRequest::Proof(Bytes::from_static(b"b"))),
            RequestOutcome::Deliver(_)
        ));
    }

    #[test]
    fn last_connection_close_clears_proof_state() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        let _ = b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof")));
        assert!(b.proofs_received.contains(&peer));

        b.on_last_connection_closed(peer);
        assert!(!b.proofs_received.contains(&peer));
    }

    #[test]
    fn admission_resets_after_close() {
        let mut b = Behaviour::with_default_protocol();
        let peer = PeerId::random();

        assert!(matches!(
            b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof"))),
            RequestOutcome::Deliver(_)
        ));
        assert!(matches!(
            b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof"))),
            RequestOutcome::Reject(_)
        ));

        b.on_last_connection_closed(peer);

        // A fresh session accepts the proof again.
        assert!(matches!(
            b.classify_request(peer, ProofRequest::Proof(Bytes::from_static(b"proof"))),
            RequestOutcome::Deliver(_)
        ));
    }

    #[test]
    fn set_proof_toggles_has_proof() {
        let mut b = Behaviour::with_default_protocol();
        assert!(!b.has_proof());
        b.set_proof(Bytes::from_static(b"proof"));
        assert!(b.has_proof());
    }
}
