use bytes::Bytes;
use malachitebft_proto::Protobuf;
use malachitebft_starknet_p2p_proto as p2p_proto;

pub struct StreamMessage {
    /// Receivers identify streams by (sender, stream_id).
    /// This means each node can allocate stream_ids independently
    /// and that many streams can be sent on a single network topic.
    pub id: Bytes,

    /// Identifies the sequence of each message in the stream starting from 0.
    pub sequence: u64,

    /// The content of this stream message
    pub content: StreamContent,
}

pub enum StreamContent {
    /// Serialized content.
    Data(Bytes),
    /// Final message.
    Fin,
}

impl Protobuf for StreamMessage {
    type Proto = p2p_proto::StreamMessage;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, malachitebft_proto::Error> {
        let content = match proto
            .message
            .ok_or_else(|| malachitebft_proto::Error::missing_field::<Self::Proto>("content"))?
        {
            p2p_proto::stream_message::Message::Content(data) => StreamContent::Data(data),
            p2p_proto::stream_message::Message::Fin(_) => StreamContent::Fin,
        };

        Ok(Self {
            id: proto.stream_id,
            sequence: proto.message_id,
            content,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, malachitebft_proto::Error> {
        Ok(Self::Proto {
            stream_id: self.id.clone(),
            message_id: self.sequence,
            message: match &self.content {
                StreamContent::Data(data) => {
                    Some(p2p_proto::stream_message::Message::Content(data.clone()))
                }
                StreamContent::Fin => {
                    Some(p2p_proto::stream_message::Message::Fin(p2p_proto::Fin {}))
                }
            },
        })
    }
}
