use core::error::Error;

use bytes::Bytes;

pub trait Codec<T>: Send + Sync + 'static {
    type Error: Error + Send;

    fn decode(&self, bytes: Bytes) -> Result<T, Self::Error>;
    fn encode(&self, msg: &T) -> Result<Bytes, Self::Error>;
}

/// Codec extension trait for types that can also compute the length of the encoded data.
pub trait HasEncodedLen<T>: Codec<T> {
    fn encoded_len(&self, msg: &T) -> Result<usize, <Self as Codec<T>>::Error>;
}
