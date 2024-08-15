use malachite_common::Context;
use malachite_consensus::GossipMsg;

pub trait NetworkCodec<Ctx: Context> {
    type Error: std::error::Error + Send + Sync + 'static;

    fn decode(&self, bytes: &[u8]) -> Result<GossipMsg<Ctx>, Self::Error>;
    fn encode(&self, msg: GossipMsg<Ctx>) -> Result<Vec<u8>, Self::Error>;
}
