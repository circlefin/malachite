use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dashmap::DashMap;
use malachitebft_metrics::prometheus::metrics::counter::Counter;
use malachitebft_metrics::prometheus::metrics::histogram::{exponential_buckets, Histogram};
use malachitebft_metrics::SharedRegistry;

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
    status_interarrival: Histogram,
    status_interarrival_normalized: Histogram, // Independent of number of peers and status update interval
    status_total: Counter,

    instant_request_sent: Arc<DashMap<u64, Instant>>,
    instant_request_received: Arc<DashMap<u64, Instant>>,
    instant_last_status_received: Arc<Mutex<Option<Instant>>>,
    status_update_interval: Duration,

    pub scoring: crate::scoring::metrics::Metrics,
}

impl Inner {
    pub fn new(status_update_interval: Duration) -> Self {
        let t = status_update_interval.as_secs_f64();
        Self {
            value_requests_sent: Counter::default(),
            value_requests_received: Counter::default(),
            value_responses_sent: Counter::default(),
            value_responses_received: Counter::default(),
            value_client_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            value_server_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            value_request_timeouts: Counter::default(),
            status_interarrival: Histogram::new(exponential_buckets(0.05 * t.max(1e-6), 1.15, 40)),
            status_interarrival_normalized: Histogram::new(exponential_buckets(0.05, 1.15, 40)),
            status_total: Counter::default(),
            instant_request_sent: Arc::new(DashMap::new()),
            instant_request_received: Arc::new(DashMap::new()),
            instant_last_status_received: Arc::new(Mutex::new(None)),
            status_update_interval,
            scoring: crate::scoring::metrics::Metrics::new(),
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

            metrics.scoring.register(registry);

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

    pub fn value_request_received(&self, height: u64) {
        self.value_requests_received.inc();
        self.instant_request_received.insert(height, Instant::now());
    }

    pub fn value_response_sent(&self, height: u64) {
        self.value_responses_sent.inc();

        if let Some((_, instant)) = self.instant_request_received.remove(&height) {
            self.value_server_latency
                .observe(instant.elapsed().as_secs_f64());
        }
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
}

impl Default for Metrics {
    fn default() -> Self {
        // Default interval of 1s.
        Self::new(Duration::from_secs(1))
    }
}
