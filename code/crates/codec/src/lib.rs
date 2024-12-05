use core::error::Error;

use bytes::Bytes;

pub trait Codec<T>: Send + Sync + 'static {
    type Error: Error + Send;

    fn decode(&self, bytes: Bytes) -> Result<T, Self::Error>;
    fn encode(&self, msg: &T) -> Result<Bytes, Self::Error>;
}
