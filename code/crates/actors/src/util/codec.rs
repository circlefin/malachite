use malachite_common::Context;
use malachite_consensus::GossipMsg;
use malachite_gossip_consensus::Bytes;
use malachite_proto::Protobuf;

use super::streaming::StreamMessage;

pub trait NetworkCodec<Ctx: Context>: Sync + Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn decode_msg(bytes: Bytes) -> Result<GossipMsg<Ctx>, Self::Error>;
    fn encode_msg(msg: GossipMsg<Ctx>) -> Result<Bytes, Self::Error>;

    fn decode_stream_msg<T>(bytes: Bytes) -> Result<StreamMessage<T>, Self::Error>
    where
        T: Protobuf;
    fn encode_stream_msg<T>(msg: StreamMessage<T>) -> Result<Bytes, Self::Error>
    where
        T: Protobuf;
}
