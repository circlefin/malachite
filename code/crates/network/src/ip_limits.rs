//! Per-IP connection limiting and throttling behaviour.
//!
//! Limits the number of **inbound** connections from a single IP address and
//! enforces a minimum delay between reconnections from the same IP. This
//! prevents DoS attacks where an attacker either:
//! - Generates many PeerIds from the same IP to exhaust connection slots, or
//! - Cycles connections rapidly (connect N → close all → reconnect N → repeat).
//!
//! Only inbound connections are limited. Outbound connections are not counted,
//! allowing nodes to connect to multiple peers behind the same NAT (e.g.,
//! validator clusters sharing a public IP).
//!
//! Tracks pending inbound connections immediately (before handshake completes)
//! to prevent resource exhaustion from incomplete handshakes.
//!
//! IPv4 connections are keyed on the full 32-bit address. IPv6 connections are
//! keyed on the `/64` prefix, matching the end-site allocation size that
//! Regional Internet Registries typically hand out. IPv4-mapped IPv6 addresses
//! are unmapped and keyed as IPv4.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use libp2p::core::Endpoint;
use libp2p::swarm::{
    ConnectionDenied, ConnectionId, FromSwarm, ListenFailure, NetworkBehaviour, THandler,
    THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use libp2p::{Multiaddr, PeerId};
use tracing::debug;

/// Minimum interval between eviction sweeps of expired throttle entries.
/// `poll()` can be called very frequently, so we avoid scanning the full
/// `ip_state` map on every call.
const EVICTION_INTERVAL: Duration = Duration::from_secs(5);

/// Per-IP state tracking active connections and reconnect cooldown.
#[derive(Debug)]
struct IpState {
    /// Number of active inbound connections from this IP.
    connections: usize,
    /// When the last connection from this IP closed (count reached 0).
    /// Set only when `connections` drops to 0; cleared when a new connection
    /// is accepted.
    last_disconnect: Option<Instant>,
}

/// Behaviour that limits connections per IP address and enforces reconnect throttling.
///
/// Tracks pending inbound connections immediately (before handshake completes)
/// to prevent attackers from exhausting resources with incomplete connections.
/// Also records when the last connection from an IP closes, rejecting new
/// connections from the same IP until a configurable cooldown has elapsed.
pub struct Behaviour {
    /// Map from ConnectionId to IP address for tracking.
    /// Includes both pending and established connections.
    connection_ips: HashMap<ConnectionId, IpAddr>,
    /// Per-IP connection count and reconnect cooldown state.
    ip_state: HashMap<IpAddr, IpState>,
    /// Maximum allowed connections per IP address.
    max_connections_per_ip: usize,
    /// Minimum time that must elapse after all connections from an IP close
    /// before a new inbound connection from that IP is accepted.
    ip_throttle_duration: Duration,
    /// IPs of persistent peers, exempt from the reconnect throttle.
    /// Refcounted: multiple persistent peers can share an IP (NAT, IPv6 /64),
    /// and the exemption is only dropped when every peer at that IP has been
    /// removed.
    persistent_ips: HashMap<IpAddr, usize>,
    /// When the last eviction sweep ran. Used to throttle sweeps so that
    /// `poll()` does not iterate `ip_state` on every event loop tick.
    last_eviction: Instant,
}

impl Behaviour {
    /// Create a new per-IP connection limiter with reconnect throttling.
    pub fn new(
        max_connections_per_ip: usize,
        ip_throttle_duration: Duration,
        persistent_peer_addrs: &[Multiaddr],
    ) -> Self {
        let mut persistent_ips: HashMap<IpAddr, usize> = HashMap::new();
        for ip in persistent_peer_addrs.iter().filter_map(extract_ip) {
            *persistent_ips.entry(ip).or_insert(0) += 1;
        }

        Self {
            connection_ips: HashMap::new(),
            ip_state: HashMap::new(),
            max_connections_per_ip,
            ip_throttle_duration,
            persistent_ips,
            last_eviction: Instant::now(),
        }
    }

    /// Drop throttle-only entries whose cooldown has expired.
    /// Entries with active connections are always retained.
    fn evict_expired(&mut self, now: Instant) {
        let throttle_duration = self.ip_throttle_duration;
        self.ip_state.retain(|_, state| {
            // Keep entries with active connections
            if state.connections > 0 {
                return true;
            }
            // Keep entries still within the throttle window
            state
                .last_disconnect
                .is_some_and(|t| now.duration_since(t) < throttle_duration)
        });
    }

    /// Register a persistent peer at this IP (exempts the IP from the throttle).
    /// Refcounted: each call must be paired with `remove_persistent_ip` once
    /// the peer is no longer persistent.
    pub fn add_persistent_ip(&mut self, ip: IpAddr) {
        let count = self.persistent_ips.entry(ip).or_insert(0);
        *count += 1;
        // On first reference, clear any pending throttle so the peer can
        // reconnect immediately.
        if *count == 1 {
            if let Some(state) = self.ip_state.get_mut(&ip) {
                if state.connections == 0 {
                    self.ip_state.remove(&ip);
                } else {
                    state.last_disconnect = None;
                }
            }
        }
    }

    /// Unregister a persistent peer at this IP. The exemption is only dropped
    /// once every peer at this IP has been removed.
    pub fn remove_persistent_ip(&mut self, ip: IpAddr) {
        if let Some(count) = self.persistent_ips.get_mut(&ip) {
            *count -= 1;
            if *count == 0 {
                self.persistent_ips.remove(&ip);
            }
        }
    }

    /// Increment connection count for an IP, tracking by connection ID.
    fn track_connection(&mut self, connection_id: ConnectionId, ip: IpAddr) {
        self.connection_ips.insert(connection_id, ip);
        let state = self.ip_state.entry(ip).or_insert(IpState {
            connections: 0,
            last_disconnect: None,
        });
        state.connections += 1;
        state.last_disconnect = None;
    }

    /// Decrement connection count when a connection closes or fails.
    /// Records the disconnect time when all connections from an IP have closed.
    fn untrack_connection(&mut self, connection_id: ConnectionId) {
        let Some(ip) = self.connection_ips.remove(&connection_id) else {
            return;
        };
        let Some(state) = self.ip_state.get_mut(&ip) else {
            return;
        };

        state.connections = state.connections.saturating_sub(1);

        if state.connections == 0 {
            // Record when the last connection from this IP closed.
            // Skip for persistent IPs (they're exempt from throttle)
            // and when throttling is disabled.
            if self.ip_throttle_duration > Duration::ZERO && !self.persistent_ips.contains_key(&ip)
            {
                state.last_disconnect = Some(Instant::now());
            } else {
                self.ip_state.remove(&ip);
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
            let count = self.ip_state.get(&ip).map(|s| s.connections).unwrap_or(0);

            if count >= self.max_connections_per_ip {
                debug!(
                    %ip,
                    count,
                    max = self.max_connections_per_ip,
                    "Rejecting inbound connection: per-IP limit exceeded"
                );
                return Err(ConnectionDenied::new(IpLimitExceeded { ip, count }));
            }

            // Enforce reconnect throttle for non-persistent IPs
            if !self.persistent_ips.contains_key(&ip) {
                if let Some(state) = self.ip_state.get(&ip) {
                    if let Some(last) = state.last_disconnect {
                        let elapsed = last.elapsed();
                        if elapsed < self.ip_throttle_duration {
                            debug!(
                                %ip,
                                ?elapsed,
                                throttle = ?self.ip_throttle_duration,
                                "Rejecting inbound connection: reconnect too soon"
                            );
                            return Err(ConnectionDenied::new(IpThrottled {
                                ip,
                                elapsed,
                                throttle_duration: self.ip_throttle_duration,
                            }));
                        }
                    }
                }
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
        match event {
            FromSwarm::ConnectionClosed(info) => {
                self.untrack_connection(info.connection_id);
            }
            FromSwarm::ListenFailure(ListenFailure { connection_id, .. }) => {
                self.untrack_connection(connection_id);
            }
            _ => {}
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
        // Sweep expired throttle-only entries periodically to bound memory
        // under sustained connection attempts from many unique IPs.
        let now = Instant::now();
        if now.duration_since(self.last_eviction) >= EVICTION_INTERVAL {
            self.evict_expired(now);
            self.last_eviction = now;
        }

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

/// Error returned when a reconnection attempt is throttled.
#[derive(Debug)]
struct IpThrottled {
    ip: IpAddr,
    elapsed: Duration,
    throttle_duration: Duration,
}

impl std::fmt::Display for IpThrottled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "reconnect throttled for {}: {:.1}s elapsed, minimum {:.1}s required",
            self.ip,
            self.elapsed.as_secs_f64(),
            self.throttle_duration.as_secs_f64()
        )
    }
}

impl std::error::Error for IpThrottled {}

/// Extract the connection-limiter key from a multiaddr.
///
/// IPv4 addresses are returned in full. IPv6 addresses are masked to their
/// `/64` prefix so that all hosts within an end-site allocation share one key.
/// IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`) are unmapped and keyed as
/// IPv4, avoiding a single bucket for the entire IPv4 space.
pub(crate) fn extract_ip(addr: &Multiaddr) -> Option<IpAddr> {
    use libp2p::multiaddr::Protocol;
    for proto in addr.iter() {
        match proto {
            Protocol::Ip4(ip) => return Some(IpAddr::V4(ip)),
            Protocol::Ip6(ip) => {
                if let Some(v4) = ip.to_ipv4_mapped() {
                    return Some(IpAddr::V4(v4));
                }
                return Some(IpAddr::V6(mask_ipv6_to_64(ip)));
            }
            _ => continue,
        }
    }
    None
}

/// Zero the host portion of an IPv6 address, keeping only the `/64` prefix.
fn mask_ipv6_to_64(ip: Ipv6Addr) -> Ipv6Addr {
    let s = ip.segments();
    Ipv6Addr::new(s[0], s[1], s[2], s[3], 0, 0, 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use libp2p::core::ConnectedPoint;
    use libp2p::swarm::{ConnectionClosed, ListenError};

    const REMOTE_ADDR: &str = "/ip4/10.0.0.1/tcp/9000";
    const LOCAL_ADDR: &str = "/ip4/127.0.0.1/tcp/8000";

    fn remote_addr() -> Multiaddr {
        REMOTE_ADDR.parse().unwrap()
    }

    fn local_addr() -> Multiaddr {
        LOCAL_ADDR.parse().unwrap()
    }

    fn remote_ip() -> IpAddr {
        "10.0.0.1".parse().unwrap()
    }

    fn listener_endpoint() -> ConnectedPoint {
        ConnectedPoint::Listener {
            local_addr: local_addr(),
            send_back_addr: remote_addr(),
        }
    }

    /// Create a behaviour with the given per-IP limit and no throttle.
    fn new_behaviour(max_connections_per_ip: usize) -> Behaviour {
        Behaviour::new(max_connections_per_ip, Duration::ZERO, &[])
    }

    /// Create a behaviour with the given per-IP limit and throttle duration.
    fn new_throttled_behaviour(max_connections_per_ip: usize, throttle: Duration) -> Behaviour {
        Behaviour::new(max_connections_per_ip, throttle, &[])
    }

    /// Track a pending inbound connection through handle_pending_inbound_connection.
    fn track_pending(b: &mut Behaviour, conn_id: ConnectionId) {
        let local = local_addr();
        let remote = remote_addr();
        b.handle_pending_inbound_connection(conn_id, &local, &remote)
            .expect("connection should be accepted");
    }

    /// Emit a ConnectionClosed event for the given connection.
    fn emit_connection_closed(b: &mut Behaviour, conn_id: ConnectionId) {
        let endpoint = listener_endpoint();
        b.on_swarm_event(FromSwarm::ConnectionClosed(ConnectionClosed {
            peer_id: PeerId::random(),
            connection_id: conn_id,
            endpoint: &endpoint,
            cause: None,
            remaining_established: 0,
        }));
    }

    /// Emit a ListenFailure event for the given connection.
    fn emit_listen_failure(b: &mut Behaviour, conn_id: ConnectionId) {
        let local = local_addr();
        let remote = remote_addr();
        let error = ListenError::Aborted;
        b.on_swarm_event(FromSwarm::ListenFailure(ListenFailure {
            local_addr: &local,
            send_back_addr: &remote,
            error: &error,
            connection_id: conn_id,
            peer_id: None,
        }));
    }

    /// Returns the connection count for an IP, or 0 if not tracked.
    fn connection_count(b: &Behaviour, ip: &IpAddr) -> usize {
        b.ip_state.get(ip).map(|s| s.connections).unwrap_or(0)
    }

    /// Returns true if the IP has a pending throttle (connections == 0, last_disconnect set).
    fn is_throttled(b: &Behaviour, ip: &IpAddr) -> bool {
        b.ip_state
            .get(ip)
            .is_some_and(|s| s.connections == 0 && s.last_disconnect.is_some())
    }

    // ── Per-IP count limit tests ──────────────────────────────────────

    #[test]
    fn counter_decremented_on_connection_closed() {
        let mut b = new_behaviour(5);
        let conn = ConnectionId::new_unchecked(1);

        track_pending(&mut b, conn);
        assert_eq!(connection_count(&b, &remote_ip()), 1);

        emit_connection_closed(&mut b, conn);
        assert_eq!(connection_count(&b, &remote_ip()), 0);
        assert!(b.connection_ips.is_empty());
    }

    #[test]
    fn counter_decremented_on_listen_failure() {
        let mut b = new_behaviour(5);
        let conn = ConnectionId::new_unchecked(1);

        track_pending(&mut b, conn);
        assert_eq!(connection_count(&b, &remote_ip()), 1);

        emit_listen_failure(&mut b, conn);
        assert_eq!(connection_count(&b, &remote_ip()), 0);
        assert!(b.connection_ips.is_empty());
    }

    #[test]
    fn connection_allowed_after_listen_failure() {
        let mut b = new_behaviour(2);
        let conn1 = ConnectionId::new_unchecked(1);
        let conn2 = ConnectionId::new_unchecked(2);

        // Fill the per-IP limit.
        track_pending(&mut b, conn1);
        track_pending(&mut b, conn2);

        // A third connection from the same IP should be denied.
        let conn3 = ConnectionId::new_unchecked(3);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn3, &local, &remote)
            .is_err());

        // Simulate a handshake failure for one connection.
        emit_listen_failure(&mut b, conn1);

        // Now a new connection from the same IP should be accepted.
        let conn4 = ConnectionId::new_unchecked(4);
        assert!(b
            .handle_pending_inbound_connection(conn4, &local, &remote)
            .is_ok());
    }

    #[test]
    fn untrack_unknown_connection_is_noop() {
        let mut b = new_behaviour(5);
        let unknown = ConnectionId::new_unchecked(999);

        // Should not panic or alter state.
        emit_listen_failure(&mut b, unknown);

        assert!(b.ip_state.is_empty());
        assert!(b.connection_ips.is_empty());
    }

    // ── Reconnect throttle tests ───────────────────────────────────────

    #[test]
    fn reconnect_rejected_within_throttle_window() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);

        // Immediate reconnect should be rejected
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &remote)
            .is_err());
    }

    #[test]
    fn reconnect_allowed_after_throttle_expires() {
        // Use a very short throttle so the test is fast
        let mut b = new_throttled_behaviour(5, Duration::from_millis(1));
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);

        // Wait for throttle to expire
        std::thread::sleep(Duration::from_millis(5));

        // Reconnect should now be allowed
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &remote)
            .is_ok());
        // After accepting, last_disconnect should be cleared (track_connection sets it to None)
        assert!(!is_throttled(&b, &remote_ip()));
    }

    #[test]
    fn persistent_ip_exempt_from_throttle() {
        let persistent_addr: Multiaddr = "/ip4/10.0.0.1/tcp/9000".parse().unwrap();
        let mut b = Behaviour::new(5, Duration::from_secs(30), &[persistent_addr]);
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect from persistent IP
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);

        // Immediate reconnect should be allowed (persistent peer exempt)
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let remote = remote_addr(); // same IP as persistent_addr: 10.0.0.1
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &remote)
            .is_ok());
    }

    #[test]
    fn add_persistent_ip_clears_throttle() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect (creates throttle entry)
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);
        assert!(is_throttled(&b, &remote_ip()));

        // Adding the IP as persistent should clear the throttle entry
        b.add_persistent_ip(remote_ip());
        assert!(!is_throttled(&b, &remote_ip()));

        // Reconnect should be allowed
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &remote)
            .is_ok());
    }

    #[test]
    fn throttle_not_recorded_when_connections_remain() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let conn1 = ConnectionId::new_unchecked(1);
        let conn2 = ConnectionId::new_unchecked(2);

        // Two connections from same IP
        track_pending(&mut b, conn1);
        track_pending(&mut b, conn2);

        // Close only one connection — count goes from 2 to 1
        emit_connection_closed(&mut b, conn1);

        // No throttle should be active (connections remain)
        assert!(!is_throttled(&b, &remote_ip()));
        assert_eq!(connection_count(&b, &remote_ip()), 1);

        // A new connection should still be allowed
        let conn3 = ConnectionId::new_unchecked(3);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn3, &local, &remote)
            .is_ok());
    }

    #[test]
    fn throttle_disabled_with_zero_duration() {
        let mut b = new_behaviour(5); // zero throttle
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);

        // No throttle entry recorded — entry fully removed
        assert!(b.ip_state.is_empty());

        // Immediate reconnect should be allowed
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let remote = remote_addr();
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &remote)
            .is_ok());
    }

    #[test]
    fn different_ips_throttled_independently() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let conn1 = ConnectionId::new_unchecked(1);

        // Connect and disconnect from IP 10.0.0.1
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);

        // A different IP should not be throttled
        let conn2 = ConnectionId::new_unchecked(2);
        let local = local_addr();
        let other_remote: Multiaddr = "/ip4/10.0.0.2/tcp/9000".parse().unwrap();
        assert!(b
            .handle_pending_inbound_connection(conn2, &local, &other_remote)
            .is_ok());
    }

    #[test]
    fn evict_expired_removes_stale_entries() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let conn1 = ConnectionId::new_unchecked(1);
        let conn2 = ConnectionId::new_unchecked(2);
        let other_remote: Multiaddr = "/ip4/10.0.0.2/tcp/9000".parse().unwrap();
        let local = local_addr();

        // IP 1: connect and disconnect (throttle entry created)
        track_pending(&mut b, conn1);
        emit_connection_closed(&mut b, conn1);
        assert!(is_throttled(&b, &remote_ip()));

        // IP 2: still actively connected
        b.handle_pending_inbound_connection(conn2, &local, &other_remote)
            .unwrap();

        // Evict using an instant far in the future, past the throttle window
        b.evict_expired(Instant::now() + Duration::from_secs(60));

        // IP 1's throttle entry should be gone
        assert!(!b.ip_state.contains_key(&remote_ip()));
        // IP 2's active connection should be preserved
        assert_eq!(connection_count(&b, &"10.0.0.2".parse().unwrap()), 1);
    }

    // ── IPv6 /64 prefix-keying tests ───────────────────────────────────

    fn multiaddr(addr: &str) -> Multiaddr {
        addr.parse().expect("valid multiaddr")
    }

    #[test]
    fn extract_ip_returns_full_ipv4_address() {
        let addr = multiaddr("/ip4/10.0.0.1/tcp/9000");
        assert_eq!(
            extract_ip(&addr),
            Some(IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1))),
        );
    }

    #[test]
    fn extract_ip_masks_ipv6_to_64_prefix() {
        let addr = multiaddr("/ip6/2001:db8:1:2:3:4:5:6/tcp/9000");
        let expected: Ipv6Addr = "2001:db8:1:2::".parse().unwrap();
        assert_eq!(extract_ip(&addr), Some(IpAddr::V6(expected)));
    }

    #[test]
    fn extract_ip_treats_ipv4_mapped_ipv6_as_ipv4() {
        let addr = multiaddr("/ip6/::ffff:10.0.0.1/tcp/9000");
        assert_eq!(
            extract_ip(&addr),
            Some(IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, 1))),
        );
    }

    #[test]
    fn extract_ip_groups_same_ipv6_64_prefix() {
        let first = multiaddr("/ip6/2001:db8::1/tcp/9000");
        let last = multiaddr("/ip6/2001:db8::ffff:ffff:ffff:ffff/tcp/9000");
        assert_eq!(extract_ip(&first), extract_ip(&last));
    }

    #[test]
    fn extract_ip_separates_different_ipv6_64_prefixes() {
        let first = multiaddr("/ip6/2001:db8:1::1/tcp/9000");
        let second = multiaddr("/ip6/2001:db8:2::1/tcp/9000");
        assert_ne!(extract_ip(&first), extract_ip(&second));
    }

    #[test]
    fn connections_from_same_ipv6_64_share_limit() {
        let mut b = new_behaviour(2);
        let local = local_addr();
        let first = multiaddr("/ip6/2001:db8::1/tcp/9000");
        let second = multiaddr("/ip6/2001:db8::2/tcp/9000");
        let third = multiaddr("/ip6/2001:db8::3/tcp/9000");

        b.handle_pending_inbound_connection(ConnectionId::new_unchecked(1), &local, &first)
            .expect("first connection accepted");
        b.handle_pending_inbound_connection(ConnectionId::new_unchecked(2), &local, &second)
            .expect("second connection accepted");
        assert!(b
            .handle_pending_inbound_connection(ConnectionId::new_unchecked(3), &local, &third)
            .is_err());
    }

    #[test]
    fn connections_from_different_ipv6_64_count_separately() {
        let mut b = new_behaviour(1);
        let local = local_addr();
        let first_prefix = multiaddr("/ip6/2001:db8:1::1/tcp/9000");
        let second_prefix = multiaddr("/ip6/2001:db8:2::1/tcp/9000");

        b.handle_pending_inbound_connection(ConnectionId::new_unchecked(1), &local, &first_prefix)
            .expect("first prefix accepted");
        b.handle_pending_inbound_connection(ConnectionId::new_unchecked(2), &local, &second_prefix)
            .expect("second prefix accepted");
    }

    // ── Persistent IP refcount tests ───────────────────────────────────

    #[test]
    fn persistent_ip_exemption_refcounted_across_removes() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        let ip = remote_ip();

        // Two persistent peers reference the same IP (NAT / shared /64).
        b.add_persistent_ip(ip);
        b.add_persistent_ip(ip);

        // First removal: still one reference, exemption must persist.
        b.remove_persistent_ip(ip);
        assert!(b.persistent_ips.contains_key(&ip));

        // Disconnect from this IP — no throttle entry should be recorded
        // because the IP is still exempt.
        let conn = ConnectionId::new_unchecked(1);
        let local = local_addr();
        let remote = remote_addr();
        b.handle_pending_inbound_connection(conn, &local, &remote)
            .unwrap();
        emit_connection_closed(&mut b, conn);
        assert!(!is_throttled(&b, &ip));

        // Second removal: refcount reaches 0, exemption dropped.
        b.remove_persistent_ip(ip);
        assert!(!b.persistent_ips.contains_key(&ip));

        // Now a disconnect should record a throttle entry.
        let conn2 = ConnectionId::new_unchecked(2);
        b.handle_pending_inbound_connection(conn2, &local, &remote)
            .unwrap();
        emit_connection_closed(&mut b, conn2);
        assert!(is_throttled(&b, &ip));
    }

    #[test]
    fn constructor_refcounts_duplicate_persistent_addrs() {
        // Two persistent peers behind the same IPv4 (different ports → same IP).
        let addr1: Multiaddr = "/ip4/10.0.0.1/tcp/9000".parse().unwrap();
        let addr2: Multiaddr = "/ip4/10.0.0.1/tcp/9001".parse().unwrap();
        let mut b = Behaviour::new(5, Duration::from_secs(30), &[addr1, addr2]);
        let ip = remote_ip();

        // One removal must not drop the exemption.
        b.remove_persistent_ip(ip);
        assert!(b.persistent_ips.contains_key(&ip));

        b.remove_persistent_ip(ip);
        assert!(!b.persistent_ips.contains_key(&ip));
    }

    #[test]
    fn remove_persistent_ip_for_unknown_is_noop() {
        let mut b = new_throttled_behaviour(5, Duration::from_secs(30));
        // No `add_persistent_ip` was ever called for this IP.
        b.remove_persistent_ip(remote_ip());
        assert!(b.persistent_ips.is_empty());
    }
}
