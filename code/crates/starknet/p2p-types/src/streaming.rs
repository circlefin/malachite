use bytes::Bytes;
use malachitebft_proto::Protobuf;
use malachitebft_starknet_p2p_proto as p2p_proto;

pub struct StreamMessage {
    /// Receivers identify streams by (sender, stream_id).
    /// This means each node can allocate stream_ids independently
    /// and that many streams can be sent on a single network topic.
    pub id: u64,

    /// Identifies the sequence of each message in the stream starting from 0.
    pub sequence: u64,

    /// The content of this stream message
    pub content: StreamContent,
}

pub enum StreamContent {
    /// Serialized content.
    Data(Bytes),
    /// Fin must be set to true.
    Fin(bool),
}

impl Protobuf for StreamMessage {
    type Proto = p2p_proto::Stream;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, malachitebft_proto::Error> {
        let content = match proto
            .content
            .ok_or_else(|| malachitebft_proto::Error::missing_field::<Self::Proto>("content"))?
        {
            p2p_proto::stream::Content::Data(data) => StreamContent::Data(data),
            p2p_proto::stream::Content::Fin(fin) => StreamContent::Fin(fin),
        };

        Ok(Self {
            id: proto.stream_id,
            sequence: proto.sequence_number,
            content,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, malachitebft_proto::Error> {
        Ok(Self::Proto {
            stream_id: self.id,
            sequence_number: self.sequence,
            content: match &self.content {
                StreamContent::Data(data) => Some(p2p_proto::stream::Content::Data(data.clone())),
                StreamContent::Fin(fin) => Some(p2p_proto::stream::Content::Fin(*fin)),
            },
        })
    }
}
