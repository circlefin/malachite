use bytes::Bytes;

pub trait NetworkCodec<T>: Send + Sync + 'static {
    type Error: core::error::Error + Send + Sync + 'static;

    fn decode(&self, bytes: Bytes) -> Result<T, Self::Error>;
    fn encode(&self, msg: T) -> Result<Bytes, Self::Error>;
}
