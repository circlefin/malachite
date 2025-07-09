use core::fmt;

use bytes::Bytes;

pub type Sequence = u64;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(pub(crate) Bytes);

impl StreamId {
    pub fn new(bytes: Bytes) -> Self {
        Self(bytes)
    }

    pub fn to_bytes(&self) -> Bytes {
        self.0.clone()
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "borsh",
    derive(::borsh::BorshSerialize, ::borsh::BorshDeserialize)
)]
pub struct StreamMessage<T> {
    /// Receivers identify streams by (sender, stream_id).
    /// This means each node can allocate stream_ids independently
    /// and that many streams can be sent on a single network topic.
    pub stream_id: StreamId,

    /// Identifies the sequence of each message in the stream starting from 0.
    pub sequence: Sequence,

    /// The content of this stream message
    pub content: StreamContent<T>,
}

impl<T> StreamMessage<T> {
    pub fn new(stream_id: StreamId, sequence: Sequence, content: StreamContent<T>) -> Self {
        Self {
            stream_id,
            sequence,
            content,
        }
    }

    pub fn is_first(&self) -> bool {
        self.sequence == 0
    }

    pub fn is_fin(&self) -> bool {
        self.content.is_fin()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "borsh",
    derive(borsh::BorshSerialize, borsh::BorshDeserialize)
)]
pub enum StreamContent<T> {
    /// Serialized content.
    Data(T),

    /// Indicates the end of the stream.
    Fin,
}

impl<T> StreamContent<T> {
    pub fn as_data(&self) -> Option<&T> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }

    pub fn into_data(self) -> Option<T> {
        match self {
            Self::Data(data) => Some(data),
            _ => None,
        }
    }

    pub fn is_fin(&self) -> bool {
        matches!(self, Self::Fin)
    }
}
