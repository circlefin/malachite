use std::time::{Duration, Instant};

use malachitebft_metrics::prometheus::metrics::counter::Counter;

use malachitebft_metrics::prometheus::metrics::gauge::Gauge;
use malachitebft_metrics::Registry;

#[derive(Debug)]
pub(crate) struct Metrics {
    /// Time at which discovery started
    start_time: Instant,
    /// Time at which the Kademlia bootstrap process finished
    initial_bootstrap_finished: Option<Instant>,
    /// Time at which initial discovery process finished
    initial_discovery_finished: Option<Instant>,

    /// Total number of discovered peers
    total_discovered: Counter,

    /// Number of active peers
    num_active_peers: Gauge,
    /// Number of active connections
    num_active_connections: Gauge,
    /// Number of outbound peers
    num_outbound_peers: Gauge,
    /// Number of outbound connections
    num_outbound_connections: Gauge,
    /// Number of inbound peers
    num_inbound_peers: Gauge,
    /// Number of inbound connections
    num_inbound_connections: Gauge,
    /// Number of ephemeral peers
    num_ephemeral_peers: Gauge,
    /// Number of ephemeral connections
    num_ephemeral_connections: Gauge,

    /// Total number of dial attempts
    total_dials: Counter,
    /// Total number of failed dial attempts
    total_failed_dials: Counter,
    /// Total number of peers request attempts
    total_peer_requests: Counter,
    /// Total number of failed peer request attempts
    total_failed_peer_requests: Counter,
    /// Total number of connect request attempts
    total_connect_requests: Counter,
    /// Total number of failed connect request attempts
    total_failed_connect_requests: Counter,
    /// Total number of rejected connect request attempts
    total_rejected_connect_requests: Counter,
}

impl Metrics {
    pub(crate) fn new(registry: &mut Registry, set_finished: bool) -> Self {
        let now = Instant::now();

        let this = Self {
            start_time: now,
            initial_bootstrap_finished: if set_finished { Some(now) } else { None },
            initial_discovery_finished: if set_finished { Some(now) } else { None },

            total_discovered: Counter::default(),

            num_active_peers: Gauge::default(),
            num_active_connections: Gauge::default(),
            num_outbound_peers: Gauge::default(),
            num_outbound_connections: Gauge::default(),
            num_inbound_peers: Gauge::default(),
            num_inbound_connections: Gauge::default(),
            num_ephemeral_peers: Gauge::default(),
            num_ephemeral_connections: Gauge::default(),

            total_dials: Counter::default(),
            total_failed_dials: Counter::default(),
            total_peer_requests: Counter::default(),
            total_failed_peer_requests: Counter::default(),
            total_connect_requests: Counter::default(),
            total_failed_connect_requests: Counter::default(),
            total_rejected_connect_requests: Counter::default(),
        };

        registry.register(
            "total_discovered",
            "Total number of discovered peers",
            this.total_discovered.clone(),
        );

        registry.register(
            "num_active_peers",
            "Number of active peers",
            this.num_active_peers.clone(),
        );

        registry.register(
            "num_active_connections",
            "Number of active connections",
            this.num_active_connections.clone(),
        );

        registry.register(
            "num_outbound_peers",
            "Number of outbound peers",
            this.num_outbound_peers.clone(),
        );

        registry.register(
            "num_outbound_connections",
            "Number of outbound connections",
            this.num_outbound_connections.clone(),
        );

        registry.register(
            "num_inbound_peers",
            "Number of inbound peers",
            this.num_inbound_peers.clone(),
        );

        registry.register(
            "num_inbound_connections",
            "Number of inbound connections",
            this.num_inbound_connections.clone(),
        );

        registry.register(
            "num_ephemeral_peers",
            "Number of ephemeral peers",
            this.num_ephemeral_peers.clone(),
        );

        registry.register(
            "num_ephemeral_connections",
            "Number of ephemeral connections",
            this.num_ephemeral_connections.clone(),
        );

        registry.register(
            "total_dials",
            "Total number of dial attempts",
            this.total_dials.clone(),
        );

        registry.register(
            "total_failed_dials",
            "Total number of failed dial attempts",
            this.total_failed_dials.clone(),
        );

        registry.register(
            "total_peer_requests",
            "Total number of peer request attempts",
            this.total_peer_requests.clone(),
        );

        registry.register(
            "total_failed_peer_requests",
            "Total number of failed peer request attempts",
            this.total_failed_peer_requests.clone(),
        );

        registry.register(
            "total_connect_requests",
            "Total number of connect request attempts",
            this.total_connect_requests.clone(),
        );

        registry.register(
            "total_failed_connect_requests",
            "Total number of failed connect request attempts",
            this.total_failed_connect_requests.clone(),
        );

        registry.register(
            "total_rejected_connect_requests",
            "Total number of rejected connect request attempts",
            this.total_rejected_connect_requests.clone(),
        );

        this
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub(crate) fn initial_bootstrap_finished(&mut self) {
        self.initial_bootstrap_finished
            .get_or_insert(Instant::now());
    }

    pub(crate) fn _initial_bootstrap_duration(&self) -> Duration {
        self.initial_bootstrap_finished
            .unwrap_or(self.start_time)
            .duration_since(self.start_time)
    }

    pub(crate) fn initial_discovery_finished(&mut self) {
        self.initial_discovery_finished
            .get_or_insert(Instant::now());
    }

    pub(crate) fn _initial_discovery_duration(&self) -> Duration {
        self.initial_discovery_finished
            .unwrap_or(self.start_time)
            .duration_since(self.start_time)
    }

    pub(crate) fn increment_total_discovered(&self) {
        self.total_discovered.inc();
    }

    pub(crate) fn set_connections_status(
        &self,
        num_active_peers: usize,
        num_active_connections: usize,
        num_outbound_peers: usize,
        num_outbound_connections: usize,
        num_inbound_peers: usize,
        num_inbound_connections: usize,
        num_ephemeral_peers: usize,
        num_ephemeral_connections: usize,
    ) {
        self.num_active_peers.set(num_active_peers as i64);
        self.num_active_connections
            .set(num_active_connections as i64);
        self.num_outbound_peers.set(num_outbound_peers as i64);
        self.num_outbound_connections
            .set(num_outbound_connections as i64);
        self.num_inbound_peers.set(num_inbound_peers as i64);
        self.num_inbound_connections
            .set(num_inbound_connections as i64);
        self.num_ephemeral_peers.set(num_ephemeral_peers as i64);
        self.num_ephemeral_connections
            .set(num_ephemeral_connections as i64);
    }

    pub(crate) fn increment_total_dials(&self) {
        self.total_dials.inc();
    }

    pub(crate) fn increment_total_failed_dials(&self) {
        self.total_failed_dials.inc();
    }

    pub(crate) fn increment_total_peer_requests(&self) {
        self.total_peer_requests.inc();
    }

    pub(crate) fn increment_total_failed_peer_requests(&self) {
        self.total_failed_peer_requests.inc();
    }

    pub(crate) fn increment_total_connect_requests(&self) {
        self.total_connect_requests.inc();
    }

    pub(crate) fn increment_total_failed_connect_requests(&self) {
        self.total_failed_connect_requests.inc();
        // A failure is also considered a rejection
        self.total_rejected_connect_requests.inc();
    }

    pub(crate) fn increment_total_rejected_connect_requests(&self) {
        self.total_rejected_connect_requests.inc();
    }

    pub(crate) fn _get_total_rejected_connect_requests(&self) -> u64 {
        self.total_rejected_connect_requests.get()
    }
}
