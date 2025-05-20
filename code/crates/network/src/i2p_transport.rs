//! Stub implementation of an I2P transport for libp2p.
//!
//! TODO: integrate with an actual I2P router (e.g., via is2fp/j4-i2p-rs) to dial
//! and listen through I2P tunnels.

use futures::future::Ready;
use tracing::error;
// Optional I2P router dependency
#[cfg(feature = "i2p")]
use j4i2prs::router_wrapper::{Wrapper, METHOD_RUN};
use libp2p::core::transport::{Transport, TransportError, TransportEvent, ListenerId, DialOpts};
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::{Multiaddr, PeerId};
// I2P tunnel control types
#[cfg(feature = "i2p")]
use j4i2prs::tunnel_control::{Tunnel, TunnelType};
use std::pin::Pin;
use std::task::{Context, Poll};

/// A placeholder transport that does nothing and rejects all addresses.
pub struct I2pTransport;

impl I2pTransport {
    /// Create a new I2P transport.
    pub fn new() -> Self {
        // Initialize I2P router and start it if enabled
        // If I2P support is enabled, start the router and a server tunnel
        #[cfg(feature = "i2p")]
        {
            // Start I2P router
            if let Ok(router) = Wrapper::create_router() {
                if let Err(e) = router.invoke_router(METHOD_RUN) {
                    error!("I2P router failed to start: {}", e);
                }
            } else {
                error!("Failed to create I2P router");
            }
            // Setup a server tunnel listening on port 5555
            let host = "127.0.0.1".to_string();
            let port = 5555;
            match j4i2prs::tunnel_control::Tunnel::new(host, port, j4i2prs::tunnel_control::TunnelType::Server) {
                Ok(tunnel) => {
                    if let Err(e) = tunnel.start(None) {
                        error!("I2P server tunnel failed to start: {}", e);
                    }
                }
                Err(e) => error!("Failed to create I2P server tunnel: {}", e),
            }
        }
        #[cfg(not(feature = "i2p"))]
        {
            error!("Building without i2p feature; I2P transport is a stub");
        }
        I2pTransport
    }
}

impl Transport for I2pTransport {
    type Output = (PeerId, StreamMuxerBox);
    type Error = std::io::Error;
    type ListenerUpgrade = Ready<Result<Self::Output, Self::Error>>;
    type Dial = Ready<Result<Self::Output, Self::Error>>;

    fn listen_on(
        &mut self,
        _id: ListenerId,
        _addr: Multiaddr,
    ) -> Result<(), TransportError<Self::Error>> {
        Err(TransportError::MultiaddrNotSupported(_addr))
    }

    fn remove_listener(&mut self, _id: ListenerId) -> bool {
        // No active listeners
        false
    }

    fn dial(
        &mut self,
        addr: Multiaddr,
        _opts: DialOpts,
    ) -> Result<Self::Dial, TransportError<Self::Error>> {
        // TODO: perform I2P dial via tunnels
        Err(TransportError::MultiaddrNotSupported(addr))
    }


    fn poll(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<TransportEvent<Self::ListenerUpgrade, Self::Error>> {
        // No events
        Poll::Pending
    }

    // Note: address_translation not supported in this libp2p version.
}