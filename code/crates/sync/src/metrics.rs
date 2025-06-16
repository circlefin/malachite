use std::ops::Deref;
use std::sync::Arc;
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

    instant_request_sent: Arc<DashMap<u64, Instant>>,
    instant_request_received: Arc<DashMap<u64, Instant>>,

    pub scoring: crate::scoring::metrics::Metrics,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            value_requests_sent: Counter::default(),
            value_requests_received: Counter::default(),
            value_responses_sent: Counter::default(),
            value_responses_received: Counter::default(),
            value_client_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            value_server_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            value_request_timeouts: Counter::default(),
            instant_request_sent: Arc::new(DashMap::new()),
            instant_request_received: Arc::new(DashMap::new()),
            scoring: crate::scoring::metrics::Metrics::new(),
        }
    }
}

impl Default for Inner {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self(Arc::new(Inner::new()))
    }

    pub fn register(registry: &SharedRegistry) -> Self {
        let metrics = Self::new();

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
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
