use malachite_common::Context;
use malachite_consensus::SignedConsensusMsg;
use malachite_gossip_consensus::Bytes;
use malachite_proto::Protobuf;

use super::streaming::StreamMessage;

pub trait NetworkCodec<Ctx: Context>: Sync + Send + 'static
where
    Self: malachite_blocksync::NetworkCodec<Ctx>,
{
    fn decode_msg(bytes: Bytes) -> Result<SignedConsensusMsg<Ctx>, Self::Error>;
    fn encode_msg(msg: SignedConsensusMsg<Ctx>) -> Result<Bytes, Self::Error>;

    fn decode_stream_msg<T>(bytes: Bytes) -> Result<StreamMessage<T>, Self::Error>
    where
        T: Protobuf;
    fn encode_stream_msg<T>(msg: StreamMessage<T>) -> Result<Bytes, Self::Error>
    where
        T: Protobuf;
}
