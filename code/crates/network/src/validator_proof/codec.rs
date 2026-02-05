//! Codec for the Validator Proof protocol.
//!
//! Uses unsigned-varint length-delimited framing for consistency with
//! other libp2p protocols (request-response, identify, etc.).

use asynchronous_codec::{FramedRead, FramedWrite};
use bytes::Bytes;
use libp2p::futures::{SinkExt, StreamExt};
use libp2p::Stream;
use unsigned_varint::codec::UviBytes;

use super::behaviour::Error;

/// Maximum size for validator proof messages.
/// Proof is ~200 bytes, so 1KB is plenty.
const MAX_MESSAGE_SIZE: usize = 1024;

/// Create a codec instance for encoding/decoding proofs.
/// Uses unsigned-varint length prefix with size limit.
fn codec() -> UviBytes {
    let mut codec = UviBytes::default();
    codec.set_max_len(MAX_MESSAGE_SIZE);
    codec
}

/// Read a validator proof from a stream.
pub async fn read_proof(stream: Stream) -> Result<Bytes, Error> {
    let mut reader = FramedRead::new(stream, codec());

    match reader.next().await {
        Some(Ok(bytes)) => Ok(bytes.into()),
        Some(Err(e)) => Err(Error::Io(e.to_string())),
        None => Err(Error::UnexpectedEof),
    }
}

/// Write a validator proof to a stream.
pub async fn write_proof(stream: Stream, proof_bytes: &Bytes) -> Result<(), Error> {
    let mut writer = FramedWrite::new(stream, codec());
    writer
        .send(proof_bytes.clone())
        .await
        .map_err(|e| Error::Io(e.to_string()))?;
    writer.close().await.map_err(|e| Error::Io(e.to_string()))?;
    Ok(())
}
