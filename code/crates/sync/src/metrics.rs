use std::ops::Deref;
use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use malachite_metrics::prometheus::metrics::counter::Counter;
use malachite_metrics::prometheus::metrics::histogram::{exponential_buckets, Histogram};
use malachite_metrics::SharedRegistry;

pub type DecidedValuesMetrics = Inner;
pub type VoteSetMetrics = Inner;

#[derive(Clone, Debug)]
pub struct Metrics(Arc<(DecidedValuesMetrics, VoteSetMetrics)>);

impl Deref for Metrics {
    type Target = (Inner, Inner);

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

    instant_request_sent: Arc<DashMap<(u64, i64), Instant>>,
    instant_request_received: Arc<DashMap<(u64, i64), Instant>>,
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
        Self(Arc::new((
            DecidedValuesMetrics::new(),
            VoteSetMetrics::new(),
        )))
    }

    fn decided_values(&self) -> &DecidedValuesMetrics {
        &self.0 .0
    }

    fn vote_set(&self) -> &VoteSetMetrics {
        &self.0 .1
    }

    pub fn register(registry: &SharedRegistry) -> Self {
        let metrics = Self::new();

        registry.with_prefix("malachite_sync", |registry| {
            // Value sync related metrics
            registry.register(
                "value_requests_sent",
                "Number of ValueSync requests sent",
                metrics.decided_values().requests_sent.clone(),
            );

            registry.register(
                "value_requests_received",
                "Number of ValueSync requests received",
                metrics.decided_values().requests_received.clone(),
            );

            registry.register(
                "value_responses_sent",
                "Number of ValueSync responses sent",
                metrics.decided_values().responses_sent.clone(),
            );

            registry.register(
                "value_responses_received",
                "Number of ValueSync responses received",
                metrics.decided_values().responses_received.clone(),
            );

            registry.register(
                "value_client_latency",
                "Interval of time between when request was sent and response was received",
                metrics.decided_values().client_latency.clone(),
            );

            registry.register(
                "value_server_latency",
                "Interval of time between when request was received and response was sent",
                metrics.decided_values().server_latency.clone(),
            );

            registry.register(
                "value_request_timeouts",
                "Number of ValueSync request timeouts",
                metrics.decided_values().request_timeouts.clone(),
            );

            registry.register(
                "vote_set_requests_sent",
                "Number of ValueSync requests sent",
                metrics.decided_values().requests_sent.clone(),
            );

            // Vote set sync related metrics
            registry.register(
                "vote_set_requests_received",
                "Number of VoteSet requests received",
                metrics.vote_set().requests_received.clone(),
            );

            registry.register(
                "vote_set_responses_sent",
                "Number of VoteSet responses sent",
                metrics.vote_set().responses_sent.clone(),
            );

            registry.register(
                "vote_set_responses_received",
                "Number of VoteSet responses received",
                metrics.vote_set().responses_received.clone(),
            );

            registry.register(
                "vote_set_client_latency",
                "Interval of time between when request was sent and response was received",
                metrics.vote_set().client_latency.clone(),
            );

            registry.register(
                "vote_set_server_latency",
                "Interval of time between when request was received and response was sent",
                metrics.vote_set().server_latency.clone(),
            );

            registry.register(
                "vote_set_timeouts",
                "Number of VoteSet request timeouts",
                metrics.vote_set().request_timeouts.clone(),
            );
        });

        metrics
    }

    pub fn decided_value_request_sent(&self, height: u64) {
        self.decided_values().requests_sent.inc();
        self.decided_values()
            .instant_request_sent
            .insert((height, -1), Instant::now());
    }

    pub fn decided_value_request_received(&self, height: u64) {
        self.decided_values().requests_received.inc();
        self.decided_values()
            .instant_request_received
            .insert((height, -1), Instant::now());
    }

    pub fn decided_value_response_sent(&self, height: u64) {
        self.decided_values().responses_sent.inc();

        if let Some((_, instant)) = self
            .decided_values()
            .instant_request_received
            .remove(&(height, -1))
        {
            self.decided_values()
                .server_latency
                .observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn decided_value_response_received(&self, height: u64) {
        self.decided_values().responses_received.inc();

        if let Some((_, instant)) = self
            .decided_values()
            .instant_request_sent
            .remove(&(height, -1))
        {
            self.decided_values()
                .client_latency
                .observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn decided_value_request_timed_out(&self, height: u64) {
        self.decided_values().request_timeouts.inc();
        self.decided_values()
            .instant_request_sent
            .remove(&(height, 0));
    }

    pub fn vote_set_request_sent(&self, height: u64, round: i64) {
        self.vote_set().requests_sent.inc();
        self.vote_set()
            .instant_request_sent
            .insert((height, round), Instant::now());
    }

    pub fn vote_set_request_received(&self, height: u64, round: i64) {
        self.vote_set().requests_received.inc();
        self.vote_set()
            .instant_request_received
            .insert((height, round), Instant::now());
    }

    pub fn vote_set_response_sent(&self, height: u64, round: i64) {
        self.vote_set().responses_sent.inc();

        if let Some((_, instant)) = self
            .vote_set()
            .instant_request_received
            .remove(&(height, round))
        {
            self.vote_set()
                .server_latency
                .observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn vote_set_response_received(&self, height: u64, round: i64) {
        self.vote_set().responses_received.inc();

        if let Some((_, instant)) = self
            .vote_set()
            .instant_request_sent
            .remove(&(height, round))
        {
            self.vote_set()
                .client_latency
                .observe(instant.elapsed().as_secs_f64());
        }
    }

    pub fn vote_set_request_timed_out(&self, height: u64, round: i64) {
        self.vote_set().request_timeouts.inc();
        self.vote_set()
            .instant_request_sent
            .remove(&(height, round));
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
