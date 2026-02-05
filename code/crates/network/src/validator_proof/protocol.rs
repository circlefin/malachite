//! Protocol handlers for sending and receiving validator proofs.

use bytes::Bytes;
use libp2p::futures::StreamExt;
use libp2p::{PeerId, Stream};
use libp2p_stream as stream;
use tokio::sync::mpsc;
use tracing::{debug, error};

use super::behaviour::{Error, Event, PROTOCOL_NAME};
use super::codec;

/// Accept and handle incoming proof streams.
pub async fn accept_incoming_streams(
    mut control: stream::Control,
    events_tx: mpsc::UnboundedSender<Event>,
) {
    let incoming = match control.accept(PROTOCOL_NAME) {
        Ok(incoming) => incoming,
        Err(error) => {
            error!(%error, "Failed to accept incoming validator proof streams");
            return;
        }
    };

    handle_incoming_streams(incoming, events_tx).await;
}

async fn handle_incoming_streams(
    mut streams: stream::IncomingStreams,
    events_tx: mpsc::UnboundedSender<Event>,
) {
    while let Some((peer, stream)) = streams.next().await {
        debug!(%peer, "Accepted incoming validator proof stream");

        let events_tx = events_tx.clone();
        tokio::spawn(async move {
            let event = recv_proof(peer, stream).await;
            let _ = events_tx.send(event);
        });
    }
}

async fn recv_proof(peer: PeerId, stream: Stream) -> Event {
    match codec::read_proof(stream).await {
        Ok(proof_bytes) => {
            debug!(%peer, proof_len = proof_bytes.len(), "Received validator proof");
            Event::ProofReceived { peer, proof_bytes }
        }
        Err(error) => {
            error!(%peer, %error, "Failed to read validator proof");
            Event::ProofReceiveFailed { peer, error }
        }
    }
}

/// Send our proof to a peer.
pub async fn send_proof(peer: PeerId, proof_bytes: Bytes, mut control: stream::Control) -> Event {
    debug!(%peer, "Opening stream to send validator proof");

    let stream = match control.open_stream(peer, PROTOCOL_NAME).await {
        Ok(stream) => stream,
        Err(error) => {
            error!(%peer, %error, "Failed to open stream for validator proof");
            return Event::ProofSendFailed {
                peer,
                error: Error::Io(error.to_string()),
            };
        }
    };

    if let Err(error) = codec::write_proof(stream, &proof_bytes).await {
        error!(%peer, %error, "Failed to write validator proof");
        return Event::ProofSendFailed { peer, error };
    }

    debug!(%peer, "Successfully sent validator proof");
    Event::ProofSent { peer }
}
