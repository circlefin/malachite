use libp2p::metrics::Registry;

#[derive(Default)]
pub struct Metrics {}

impl Metrics {
    pub fn new(_registry: &mut Registry) -> Self {
        Self {}
    }
}
