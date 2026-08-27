//! Codec for the Validator Proof protocol.
//!
//! The protocol is carried over `request_response` but remains a one-way
//! message on the wire: the request is the length-delimited proof, and the
//! response carries no bytes.
//!
//! ```text
//! Request:  [unsigned-varint length][proof bytes]
//! Response: <no bytes>
//! ```
//!
//! The request framing is unsigned-varint length-delimited, matching the other
//! libp2p protocols. The response is zero-length: it lets the request_response
//! handler complete and close its stream, and is not a delivery acknowledgement.
//!
//! `read_request` never returns an error: a framing error, oversized frame,
//! truncated payload, or a read past [`READ_TIMEOUT`] becomes an in-band
//! [`ProofRequest::Malformed`]. request_response discards a pre-delivery read
//! error internally, so a rejection must reach the behaviour as a request.

use std::io;
use std::time::Duration;

use async_trait::async_trait;
use asynchronous_codec::{FramedRead, FramedWrite};
use bytes::Bytes;
use libp2p::futures::io::{AsyncRead, AsyncWrite};
use libp2p::futures::{SinkExt, StreamExt};
use libp2p::request_response;
use libp2p::StreamProtocol;
use unsigned_varint::codec::UviBytes;

/// Maximum size for validator proof messages.
/// Proof is ~200 bytes, so 1KB is plenty.
const MAX_MESSAGE_SIZE: usize = 1024;

/// Bound on reading a request off the wire. Stays below the request_response
/// handler timeout so a stalled read becomes an in-band [`ProofRequest::Malformed`].
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// An inbound validator-proof request.
#[derive(Clone, Debug)]
pub enum ProofRequest {
    /// A well-framed proof payload.
    Proof(Bytes),
    /// The request could not be read (framing error, oversized, truncated, or timed out).
    Malformed,
}

/// Create the unsigned-varint length-delimited framing used for the request.
fn framing() -> UviBytes {
    let mut framing = UviBytes::default();
    framing.set_max_len(MAX_MESSAGE_SIZE);
    framing
}

/// Request/response codec for the validator-proof protocol.
///
/// The request is the length-delimited proof; the response is empty.
#[derive(Clone, Default)]
pub struct Codec;

#[async_trait]
impl request_response::Codec for Codec {
    type Protocol = StreamProtocol;
    type Request = ProofRequest;
    type Response = ();

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut framed = FramedRead::new(io, framing());
        let request = match tokio::time::timeout(READ_TIMEOUT, framed.next()).await {
            Ok(Some(Ok(bytes))) => ProofRequest::Proof(bytes.into()),
            _ => ProofRequest::Malformed,
        };
        Ok(request)
    }

    /// The response carries no bytes; return without reading.
    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        _: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        Ok(())
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // Only proofs are ever sent; `Malformed` is an inbound-only outcome.
        let ProofRequest::Proof(bytes) = req else {
            debug_assert!(false, "write_request called with a Malformed request");
            return Ok(());
        };
        let mut framed = FramedWrite::new(io, framing());
        framed.send(bytes).await?;
        framed.close().await
    }

    /// Write nothing: the response is a zero-length completion signal.
    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        _: &mut T,
        _: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::futures::io::Cursor;
    use request_response::Codec as _;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    fn protocol() -> StreamProtocol {
        StreamProtocol::new("/malachitebft-validator-proof/v1")
    }

    /// A reader that never yields data, to exercise the read timeout.
    struct NeverReady;

    impl AsyncRead for NeverReady {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn request_round_trips_through_uvi_framing() {
        let proof = Bytes::from_static(b"validator-proof-bytes");

        let mut buf = Vec::new();
        Codec
            .write_request(
                &protocol(),
                &mut Cursor::new(&mut buf),
                ProofRequest::Proof(proof.clone()),
            )
            .await
            .unwrap();

        // Wire is a varint length prefix followed by the raw proof bytes.
        assert_eq!(buf[0] as usize, proof.len());
        assert_eq!(&buf[1..], proof.as_ref());

        let decoded = Codec
            .read_request(&protocol(), &mut Cursor::new(buf))
            .await
            .unwrap();
        assert!(matches!(decoded, ProofRequest::Proof(b) if b == proof));
    }

    #[tokio::test]
    async fn oversized_request_is_malformed() {
        let mut framed = FramedWrite::new(Vec::new(), {
            let mut f = UviBytes::default();
            f.set_max_len(MAX_MESSAGE_SIZE + 1);
            f
        });
        framed
            .send(Bytes::from(vec![0u8; MAX_MESSAGE_SIZE + 1]))
            .await
            .unwrap();
        framed.close().await.unwrap();
        let oversized = framed.into_inner();

        let decoded = Codec
            .read_request(&protocol(), &mut Cursor::new(oversized))
            .await
            .unwrap();
        assert!(matches!(decoded, ProofRequest::Malformed));
    }

    #[tokio::test]
    async fn empty_stream_is_malformed() {
        let decoded = Codec
            .read_request(&protocol(), &mut Cursor::new(Vec::new()))
            .await
            .unwrap();
        assert!(matches!(decoded, ProofRequest::Malformed));
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_read_times_out_as_malformed() {
        let decoded = Codec
            .read_request(&protocol(), &mut NeverReady)
            .await
            .unwrap();
        assert!(matches!(decoded, ProofRequest::Malformed));
    }

    #[tokio::test]
    async fn response_writes_no_bytes() {
        let mut buf = Vec::new();
        Codec
            .write_response(&protocol(), &mut Cursor::new(&mut buf), ())
            .await
            .unwrap();
        assert!(buf.is_empty());
    }

    #[tokio::test]
    async fn response_read_returns_without_consuming() {
        Codec
            .read_response(&protocol(), &mut Cursor::new(Vec::new()))
            .await
            .unwrap();
    }
}
