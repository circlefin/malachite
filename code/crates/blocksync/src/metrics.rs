use libp2p::metrics::Registry;

pub struct Metrics {}

impl Metrics {
    pub fn new(_registry: &mut Registry) -> Self {
        Self {}
    }
}
