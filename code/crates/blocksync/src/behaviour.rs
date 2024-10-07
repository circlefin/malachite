use std::task::Poll;

use libp2p::metrics::Registry;
use libp2p::request_response::{self as rpc, ProtocolSupport};
use libp2p::swarm::{self, NetworkBehaviour};
use libp2p::StreamProtocol;

use malachite_common::{Context, SignedVote};

use crate::codec::RpcCodec;
use crate::NetworkCodec;

// use crate::Metrics;

#[derive(Debug)]
pub struct Request<Ctx: Context> {
    pub height: Ctx::Height,
}

#[derive(Debug)]
pub struct Response<Ctx: Context> {
    pub height: Ctx::Height,
    pub commits: Vec<SignedVote<Ctx>>,
    pub block_bytes: Vec<u8>,
}

pub struct Behaviour<Ctx: Context, N: NetworkCodec<Ctx>> {
    rpc: rpc::Behaviour<RpcCodec<Ctx, N>>,
}

pub type Event<Ctx> = rpc::Event<Request<Ctx>, Response<Ctx>>;

impl<Ctx, N> Behaviour<Ctx, N>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    pub const PROTOCOL: [(StreamProtocol, ProtocolSupport); 1] = [(
        StreamProtocol::new("/malachite-blocksync/v1beta1"),
        ProtocolSupport::Full,
    )];

    pub fn new() -> Self {
        let config = rpc::Config::default();
        Self {
            rpc: rpc::Behaviour::with_codec(RpcCodec::default(), Self::PROTOCOL, config),
            // metrics: None,
        }
    }

    pub fn new_with_metrics(_registry: &mut Registry) -> Self {
        let config = rpc::Config::default();
        Self {
            rpc: rpc::Behaviour::new(Self::PROTOCOL, config),
            // metrics: Some(Metrics::new(registry)),
        }
    }
}

impl<Ctx, N> Default for Behaviour<Ctx, N>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx, N> NetworkBehaviour for Behaviour<Ctx, N>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    type ConnectionHandler =
        <rpc::Behaviour<RpcCodec<Ctx, N>> as NetworkBehaviour>::ConnectionHandler;

    type ToSwarm = <rpc::Behaviour<RpcCodec<Ctx, N>> as NetworkBehaviour>::ToSwarm;

    fn handle_established_inbound_connection(
        &mut self,
        _connection_id: swarm::ConnectionId,
        peer: libp2p::PeerId,
        local_addr: &libp2p::Multiaddr,
        remote_addr: &libp2p::Multiaddr,
    ) -> Result<swarm::THandler<Self>, swarm::ConnectionDenied> {
        self.rpc.handle_established_inbound_connection(
            _connection_id,
            peer,
            local_addr,
            remote_addr,
        )
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: swarm::ConnectionId,
        peer: libp2p::PeerId,
        addr: &libp2p::Multiaddr,
        role_override: libp2p::core::Endpoint,
        port_use: libp2p::core::transport::PortUse,
    ) -> Result<swarm::THandler<Self>, swarm::ConnectionDenied> {
        self.rpc.handle_established_outbound_connection(
            connection_id,
            peer,
            addr,
            role_override,
            port_use,
        )
    }

    fn on_swarm_event(&mut self, event: swarm::FromSwarm) {
        self.rpc.on_swarm_event(event)
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: libp2p::PeerId,
        connection_id: swarm::ConnectionId,
        event: swarm::THandlerOutEvent<Self>,
    ) {
        self.rpc
            .on_connection_handler_event(peer_id, connection_id, event)
    }

    fn poll(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<swarm::ToSwarm<Self::ToSwarm, swarm::THandlerInEvent<Self>>> {
        self.rpc.poll(cx)
    }
}
