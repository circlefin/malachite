pub type StreamId = u64;
pub type Sequence = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
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
pub enum StreamContent<T> {
    /// Serialized content.
    Data(T),

    /// Fin must be set to true.
    Fin(bool),
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
        matches!(self, Self::Fin(true))
    }
}
