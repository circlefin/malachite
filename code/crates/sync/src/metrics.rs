use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use malachitebft_metrics::prometheus::encoding::{
    EncodeLabelSet, EncodeLabelValue, LabelValueEncoder,
};
use malachitebft_metrics::prometheus::metrics::counter::Counter;
use malachitebft_metrics::prometheus::metrics::family::Family;
use malachitebft_metrics::prometheus::metrics::gauge::Gauge;
use malachitebft_metrics::prometheus::metrics::histogram::{exponential_buckets, Histogram};
use malachitebft_metrics::SharedRegistry;

// Make prometheus_client available for the EncodeLabelSet derive macro.
use malachitebft_metrics::prometheus as prometheus_client;

use crate::{InboundFailureReason, InboundRequestId, OutboundFailureReason};

impl EncodeLabelValue for OutboundFailureReason {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        let s = match self {
            OutboundFailureReason::DialFailure => "dial_failure",
            OutboundFailureReason::ConnectionClosed => "connection_closed",
            OutboundFailureReason::Timeout => "timeout",
            OutboundFailureReason::UnsupportedProtocols => "unsupported_protocols",
            OutboundFailureReason::Io => "io",
        };
        std::fmt::Write::write_str(encoder, s)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct OutboundFailureReasonLabel {
    pub reason: OutboundFailureReason,
}

impl OutboundFailureReasonLabel {
    pub fn new(reason: OutboundFailureReason) -> Self {
        Self { reason }
    }
}

impl EncodeLabelValue for InboundFailureReason {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        let s = match self {
            InboundFailureReason::RequesterDisconnected => "requester_disconnected",
            InboundFailureReason::HostStallTimeout => "host_stall_timeout",
        };
        std::fmt::Write::write_str(encoder, s)
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct InboundFailureReasonLabel {
    pub reason: InboundFailureReason,
}

impl InboundFailureReasonLabel {
    pub fn new(reason: InboundFailureReason) -> Self {
        Self { reason }
    }
}

#[derive(Clone, Debug)]
pub struct Metrics(Arc<Inner>);

impl Deref for Metrics {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
pub struct Inner {
    value_requests_sent: Counter,
    value_requests_received: Counter,
    value_responses_sent: Counter,
    value_responses_received: Counter,
    value_client_latency: Histogram,
    value_server_latency: Histogram,
    value_request_timeouts: Counter,
    value_local_transient_errors: Counter,
    value_request_failures: Family<OutboundFailureReasonLabel, Counter>,
    value_inbound_request_failures: Family<InboundFailureReasonLabel, Counter>,
    status_interarrival: Histogram,
    status_interarrival_normalized: Histogram, // Independent of number of peers and status update interval
    status_total: Counter,

    instant_request_sent: Arc<DashMap<u64, Instant>>,
    instant_request_received: Arc<DashMap<InboundRequestId, Instant>>,
    instant_last_status_received: Arc<Mutex<Option<Instant>>>,
    status_update_interval: Duration,

    pub scoring: crate::scoring::metrics::Metrics,

    /// Number of heights in the sync input queue
    pub sync_queue_heights: Gauge,

    /// Number of inputs in the sync input queue across all heights
    pub sync_queue_size: Gauge,
}

impl Inner {
    pub fn new(status_update_interval: Duration) -> Self {
        let t = status_update_interval.as_secs_f64();
        let value_client_latency_buckets = vec![
            0.001, 0.002, 0.003, 0.004, 0.006, 0.008, 0.012, 0.016, 0.020, 0.024, 0.028, 0.032,
            0.040, 0.050, 0.060, 0.070, 0.080, 0.100, 0.150, 0.250, 0.500, 1.0, 2.0, 4.0, 8.0,
            16.0, 32.0, 64.0,
        ];
        debug_assert!(
            value_client_latency_buckets
                .windows(2)
                .all(|window| window[0] < window[1]),
            "value_client_latency buckets must be strictly ascending"
        );

        Self {
            value_requests_sent: Counter::default(),
            value_requests_received: Counter::default(),
            value_responses_sent: Counter::default(),
            value_responses_received: Counter::default(),
            // Resolve the observed 1-4ms, 20-32ms, and 60-80ms client latency bands,
            // with coarser buckets for multi-second degradation.
            value_client_latency: Histogram::new(value_client_latency_buckets),
            // 1ms to 65.536s covers typical 1-4ms serving latency and degraded conditions.
            value_server_latency: Histogram::new(exponential_buckets(0.001, 2.0, 17)),
            value_request_timeouts: Counter::default(),
            value_local_transient_errors: Counter::default(),
            value_request_failures: Family::default(),
            value_inbound_request_failures: Family::default(),
            status_interarrival: Histogram::new(exponential_buckets(0.05 * t.max(1e-6), 1.15, 40)),
            status_interarrival_normalized: Histogram::new(exponential_buckets(0.05, 1.15, 40)),
            status_total: Counter::default(),
            instant_request_sent: Arc::new(DashMap::new()),
            instant_request_received: Arc::new(DashMap::new()),
            instant_last_status_received: Arc::new(Mutex::new(None)),
            status_update_interval,
            scoring: crate::scoring::metrics::Metrics::new(),
            sync_queue_heights: Gauge::default(),
            sync_queue_size: Gauge::default(),
        }
    }
}

impl Metrics {
    pub fn new(status_update_interval: Duration) -> Self {
        Self(Arc::new(Inner::new(status_update_interval)))
    }

    pub fn register(registry: &SharedRegistry, status_update_interval: Duration) -> Self {
        let metrics = Self::new(status_update_interval);

        registry.with_prefix("malachitebft_sync", |registry| {
            // Value sync related metrics
            registry.register(
                "value_requests_sent",
                "Number of ValueSync requests sent",
                metrics.value_requests_sent.clone(),
            );

            registry.register(
                "value_requests_received",
                "Number of ValueSync requests received",
                metrics.value_requests_received.clone(),
            );

            registry.register(
                "value_responses_sent",
                "Number of ValueSync responses sent",
                metrics.value_responses_sent.clone(),
            );

            registry.register(
                "value_responses_received",
                "Number of ValueSync responses received",
                metrics.value_responses_received.clone(),
            );

            registry.register(
                "value_client_latency",
                "Interval of time between when request was sent and response was received",
                metrics.value_client_latency.clone(),
            );

            registry.register(
                "value_server_latency",
                "Interval of time between when request was received and response was sent",
                metrics.value_server_latency.clone(),
            );

            registry.register(
                "value_request_timeouts",
                "Number of ValueSync request timeouts",
                metrics.value_request_timeouts.clone(),
            );

            registry.register(
                "value_local_transient_errors",
                "Number of local/transient errors while processing synced values (no peer penalized or excluded)",
                metrics.value_local_transient_errors.clone(),
            );

            registry.register(
                "value_request_failures",
                "Number of ValueSync requests reported as failed by the network layer, labeled by reason",
                metrics.value_request_failures.clone(),
            );

            registry.register(
                "value_inbound_request_failures",
                "Number of inbound ValueSync requests dropped without a response, labeled by reason",
                metrics.value_inbound_request_failures.clone(),
            );

            metrics.scoring.register(registry);

            registry.register(
                "sync_queue_heights",
                "Number of heights in the sync input queue",
                metrics.sync_queue_heights.clone(),
            );

            registry.register(
                "sync_queue_size",
                "Number of inputs in the sync input queue across all heights",
                metrics.sync_queue_size.clone(),
            );

            registry.register(
                "status_interarrival",
                "Status updates interarrival histogram (any peer)",
                metrics.status_interarrival.clone(),
            );

            registry.register(
                "status_interarrival_normalized",
                "Status updates interarrival histogram (any peer) normalized to have a mean of 1",
                metrics.status_interarrival_normalized.clone(),
            );
            registry.register(
                "status_total",
                "Total number of status updates received",
                metrics.status_total.clone(),
            );
        });

        metrics
    }

    pub fn value_request_sent(&self, height: u64) {
        self.value_requests_sent.inc();
        self.instant_request_sent.insert(height, Instant::now());
    }

    pub fn value_request_received(&self, request_id: &InboundRequestId) {
        self.value_requests_received.inc();
        self.instant_request_received
            .insert(request_id.clone(), Instant::now());
    }

    pub fn value_response_sent(&self, request_id: &InboundRequestId) {
        self.value_responses_sent.inc();

        if let Some((_, instant)) = self.instant_request_received.remove(request_id) {
            self.value_server_latency
                .observe(instant.elapsed().as_secs_f64());
        }
    }

    #[cfg(test)]
    pub(crate) fn value_server_latency_observation_count(&self) -> u64 {
        use malachitebft_metrics::prometheus::encoding::text::encode;
        use malachitebft_metrics::Registry;

        let mut registry = Registry::default();
        registry.register(
            "value_server_latency",
            "ValueSync server latency",
            self.value_server_latency.clone(),
        );

        let mut output = String::new();
        encode(&mut output, &registry).expect("metric encoding should succeed");

        output
            .lines()
            .find_map(|line| line.strip_prefix("value_server_latency_count "))
            .expect("encoded histogram should contain an observation count")
            .parse()
            .expect("histogram observation count should be an integer")
    }

    pub fn value_response_received(&self, height: u64) -> Option<Duration> {
        self.value_responses_received.inc();

        if let Some((_, instant_request_sent)) = self.instant_request_sent.remove(&height) {
            let latency = instant_request_sent.elapsed();
            self.value_client_latency.observe(latency.as_secs_f64());
            Some(latency)
        } else {
            None
        }
    }

    pub fn value_request_timed_out(&self, height: u64) {
        self.value_request_timeouts.inc();
        self.instant_request_sent.remove(&height);
    }

    /// A synced value could not be processed due to a local/transient failure
    /// (e.g. the execution layer being temporarily unavailable). No peer is
    /// penalized or excluded; the batch is re-requested.
    pub fn value_local_transient_error(&self) {
        self.value_local_transient_errors.inc();
    }

    pub fn value_request_failed(&self, reason: OutboundFailureReason, height: u64) {
        self.value_request_failures
            .get_or_create(&OutboundFailureReasonLabel::new(reason))
            .inc();
        self.instant_request_sent.remove(&height);
    }

    /// A pending inbound request was dropped before a response was sent (the
    /// requester disconnected, or the host stalled past the inbound budget).
    pub fn value_inbound_request_failed(
        &self,
        request_id: &InboundRequestId,
        reason: InboundFailureReason,
    ) {
        self.instant_request_received.remove(request_id);
        self.value_inbound_request_failures
            .get_or_create(&InboundFailureReasonLabel::new(reason))
            .inc();
    }

    pub fn status_received(&self, n_peers: u64) {
        self.status_total.inc();
        let now = Instant::now();

        let mut last_recv_guard = self.instant_last_status_received.lock().unwrap();
        if let Some(prev) = *last_recv_guard {
            let delta = now.duration_since(prev).as_secs_f64();
            self.status_interarrival.observe(delta);

            if n_peers > 0 {
                // Observe normalized metric (delta / (T/N))
                let mu = self.status_update_interval.as_secs_f64() / (n_peers as f64);
                if mu > 0.0 {
                    let ratio = delta / mu;
                    self.status_interarrival_normalized.observe(ratio);
                }
            }
        }
        *last_recv_guard = Some(now);
    }

    pub fn sync_queue_updated(&self, heights: usize, size: usize) {
        self.sync_queue_heights.set(heights as _);
        self.sync_queue_size.set(size as _);
    }
}

impl Default for Metrics {
    fn default() -> Self {
        // Default interval of 1s.
        Self::new(Duration::from_secs(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_request_failed_metric_registers_and_increments_per_reason() {
        let registry = SharedRegistry::global().with_moniker("test_value_request_failed_metric");
        let metrics = Metrics::register(&registry, Duration::from_secs(1));

        // Each variant should produce a distinct, independently incrementing label.
        for reason in [
            OutboundFailureReason::DialFailure,
            OutboundFailureReason::ConnectionClosed,
            OutboundFailureReason::Timeout,
            OutboundFailureReason::UnsupportedProtocols,
            OutboundFailureReason::Io,
        ] {
            metrics.value_request_failed(reason, 100);
        }

        // Increment DialFailure a second time to confirm per-label accumulation.
        metrics.value_request_failed(OutboundFailureReason::DialFailure, 101);

        let count_for = |reason: OutboundFailureReason| -> u64 {
            metrics
                .value_request_failures
                .get_or_create(&OutboundFailureReasonLabel::new(reason))
                .get()
        };

        assert_eq!(count_for(OutboundFailureReason::DialFailure), 2);
        assert_eq!(count_for(OutboundFailureReason::ConnectionClosed), 1);
        assert_eq!(count_for(OutboundFailureReason::Timeout), 1);
        assert_eq!(count_for(OutboundFailureReason::UnsupportedProtocols), 1);
        assert_eq!(count_for(OutboundFailureReason::Io), 1);
    }

    #[test]
    fn test_value_inbound_request_failed_metric_registers_and_increments_per_reason() {
        let registry =
            SharedRegistry::global().with_moniker("test_value_inbound_request_failed_metric");
        let metrics = Metrics::register(&registry, Duration::from_secs(1));

        for (request_id, reason) in [
            (
                InboundRequestId::new("requester-disconnected"),
                InboundFailureReason::RequesterDisconnected,
            ),
            (
                InboundRequestId::new("host-stall-timeout"),
                InboundFailureReason::HostStallTimeout,
            ),
        ] {
            metrics.value_request_received(&request_id);
            metrics.value_inbound_request_failed(&request_id, reason);
        }

        // Increment RequesterDisconnected a second time to confirm per-label accumulation.
        let request_id = InboundRequestId::new("another-requester-disconnected");
        metrics.value_request_received(&request_id);
        metrics
            .value_inbound_request_failed(&request_id, InboundFailureReason::RequesterDisconnected);

        let count_for = |reason: InboundFailureReason| -> u64 {
            metrics
                .value_inbound_request_failures
                .get_or_create(&InboundFailureReasonLabel::new(reason))
                .get()
        };

        assert_eq!(count_for(InboundFailureReason::RequesterDisconnected), 2);
        assert_eq!(count_for(InboundFailureReason::HostStallTimeout), 1);
        assert!(metrics.instant_request_received.is_empty());
    }
}
