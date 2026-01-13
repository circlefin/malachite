//! Per-IP connection limiting behaviour.
//!
//! Limits the number of **inbound** connections from a single IP address to
//! prevent DoS attacks where an attacker generates many PeerIds from the same
//! IP to exhaust connection slots.
//!
//! Only inbound connections are limited. Outbound connections are not counted,
//! allowing nodes to connect to multiple peers behind the same NAT (e.g.,
//! validator clusters sharing a public IP).
//!
//! Tracks pending inbound connections immediately (before handshake completes)
//! to prevent resource exhaustion from incomplete handshakes.

use std::collections::HashMap;
use std::net::IpAddr;
use std::task::{Context, Poll};

use libp2p::core::Endpoint;
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler, THandlerInEvent,
    THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use tracing::debug;

/// Behaviour that limits connections per IP address.
///
/// Tracks pending inbound connections immediately (before handshake completes)
/// to prevent attackers from exhausting resources with incomplete connections.
pub struct Behaviour {
    /// Map from ConnectionId to IP address for tracking.
    /// Includes both pending and established connections.
    connection_ips: HashMap<ConnectionId, IpAddr>,
    /// Count of connections per IP address (derived from connection_ips).
    connections_per_ip: HashMap<IpAddr, usize>,
    /// Maximum allowed connections per IP address.
    max_connections_per_ip: usize,
}

impl Behaviour {
    /// Create a new per-IP connection limiter.
    pub fn new(max_connections_per_ip: usize) -> Self {
        Self {
            connection_ips: HashMap::new(),
            connections_per_ip: HashMap::new(),
            max_connections_per_ip,
        }
    }

    /// Increment connection count for an IP, tracking by connection ID.
    fn track_connection(&mut self, connection_id: ConnectionId, ip: IpAddr) {
        self.connection_ips.insert(connection_id, ip);
        *self.connections_per_ip.entry(ip).or_insert(0) += 1;
    }

    /// Decrement connection count when a connection closes or fails.
    fn untrack_connection(&mut self, connection_id: ConnectionId) {
        if let Some(ip) = self.connection_ips.remove(&connection_id) {
            if let Some(count) = self.connections_per_ip.get_mut(&ip) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.connections_per_ip.remove(&ip);
                }
            }
        }
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
    type ToSwarm = std::convert::Infallible;

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        if let Some(ip) = extract_ip(remote_addr) {
            let count = self.connections_per_ip.get(&ip).copied().unwrap_or(0);
            if count >= self.max_connections_per_ip {
                debug!(
                    %ip,
                    count,
                    max = self.max_connections_per_ip,
                    "Rejecting inbound connection: per-IP limit exceeded"
                );
                return Err(ConnectionDenied::new(IpLimitExceeded { ip, count }));
            }
            // Track immediately to prevent race conditions with concurrent connections
            self.track_connection(connection_id, ip);
        }
        Ok(())
    }

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _local_addr: &Multiaddr,
        _remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        // Already tracked in handle_pending_inbound_connection
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn handle_established_outbound_connection(
        &mut self,
        _connection_id: ConnectionId,
        _peer: PeerId,
        _addr: &Multiaddr,
        _role_override: Endpoint,
        _port_use: libp2p::core::transport::PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(libp2p::swarm::dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm<'_>) {
        if let FromSwarm::ConnectionClosed(info) = event {
            // Untrack inbound connections (outbound are not tracked)
            // Handles both pending failures and normal closes
            self.untrack_connection(info.connection_id);
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _peer_id: PeerId,
        _connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        // dummy::ConnectionHandler produces no events
        match event {}
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}

/// Error returned when the per-IP connection limit is exceeded.
#[derive(Debug)]
struct IpLimitExceeded {
    ip: IpAddr,
    count: usize,
}

impl std::fmt::Display for IpLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "per-IP connection limit exceeded for {}: {} connections",
            self.ip, self.count
        )
    }
}

impl std::error::Error for IpLimitExceeded {}

/// Extract IP address from a multiaddr.
fn extract_ip(addr: &Multiaddr) -> Option<IpAddr> {
    use libp2p::multiaddr::Protocol;
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => return Some(IpAddr::V6(ip)),
            _ => continue,
        }
    }
    None
}
