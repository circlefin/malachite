use std::ops::Deref;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use malachite_metrics::prometheus::metrics::counter::Counter;
use malachite_metrics::prometheus::metrics::histogram::{exponential_buckets, Histogram};
use malachite_metrics::SharedRegistry;

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
    requests_sent: Counter,
    requests_received: Counter,
    responses_sent: Counter,
    responses_received: Counter,
    client_latency: Histogram,
    server_latency: Histogram,
    request_timeouts: Counter,

    instant_request_sent: Arc<DashMap<u64, Instant>>,
    instant_request_received: Arc<DashMap<u64, Instant>>,
}

impl Inner {
    pub fn new() -> Self {
        Self {
            requests_sent: Counter::default(),
            requests_received: Counter::default(),
            responses_sent: Counter::default(),
            responses_received: Counter::default(),
            client_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            server_latency: Histogram::new(exponential_buckets(0.1, 2.0, 20)),
            request_timeouts: Counter::default(),
            instant_request_sent: Arc::new(DashMap::new()),
            instant_request_received: Arc::new(DashMap::new()),
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

        registry.with_prefix("malachite_blocksync", |registry| {
            registry.register(
                "requests_sent",
                "Number of BlockSync requests sent",
                metrics.requests_sent.clone(),
            );

            registry.register(
                "requests_received",
                "Number of BlockSync requests received",
                metrics.requests_received.clone(),
            );

            registry.register(
                "responses_sent",
                "Number of BlockSync responses sent",
                metrics.responses_sent.clone(),
            );

            registry.register(
                "responses_received",
                "Number of BlockSync responses received",
                metrics.responses_received.clone(),
            );

            registry.register(
                "client_latency",
                "Interval of time between when request was sent and response was received",
                metrics.client_latency.clone(),
            );

            registry.register(
                "server_latency",
                "Interval of time between when request was received and response was sent",
                metrics.server_latency.clone(),
            );

            registry.register(
                "timeouts",
                "Number of BlockSync request timeouts",
                metrics.request_timeouts.clone(),
            );
        });

        metrics
    }

    pub fn request_sent(&self, height: u64) {
        self.requests_sent.inc();
        self.instant_request_sent.insert(height, Instant::now());
    }

    pub fn response_received(&self, height: u64) {
        self.responses_received.inc();

        if let Some((_, instant)) = self.instant_request_sent.remove(&height) {
            self.client_latency.observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn request_received(&self, height: u64) {
        self.requests_received.inc();
        self.instant_request_received.insert(height, Instant::now());
    }

    pub fn response_sent(&self, height: u64) {
        self.responses_sent.inc();

        if let Some((_, instant)) = self.instant_request_received.remove(&height) {
            self.server_latency.observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn request_timed_out(&self, height: u64) {
        self.request_timeouts.inc();
        self.instant_request_sent.remove(&height);
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
