use std::fmt::Write;

use malachitebft_metrics::prometheus::encoding::{
    EncodeLabelSet, EncodeLabelValue, LabelValueEncoder,
};
use malachitebft_metrics::prometheus::metrics::family::Family;
use malachitebft_metrics::prometheus::metrics::histogram::{linear_buckets, Histogram};
use malachitebft_metrics::Registry;
use malachitebft_peer::PeerId;

use malachitebft_metrics::prometheus as prometheus_client;

use super::Score;

/// This wrapper allows us to derive `AsLabelValue` for `PeerId` without
/// running into Rust orphan rules, cf. <https://rust-lang.github.io/chalk/book/clauses/coherence.html>
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct AsLabelValue<T>(T);

impl EncodeLabelValue for AsLabelValue<PeerId> {
    fn encode(&self, encoder: &mut LabelValueEncoder) -> Result<(), std::fmt::Error> {
        encoder.write_fmt(format_args!("{}", self.0))
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
pub struct PeerLabel {
    peer_id: AsLabelValue<PeerId>,
}

impl PeerLabel {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id: AsLabelValue(peer_id),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Metrics {
    pub scores: Family<PeerLabel, Histogram>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            scores: Family::new_with_constructor(|| Histogram::new(linear_buckets(0.0, 0.05, 20))),
        }
    }

    pub fn register(&self, registry: &mut Registry) {
        registry.register("scores", "Peer scores", self.scores.clone());
    }

    pub fn observe_score(&self, peer_id: PeerId, score: Score) {
        self.scores
            .get_or_create(&PeerLabel::new(peer_id))
            .observe(score);
    }
}
