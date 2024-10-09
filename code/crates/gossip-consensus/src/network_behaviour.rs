use either::Either;
use libp2p::swarm::derive_prelude::*;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{gossipsub, identify, ping};
use libp2p_broadcast as broadcast;

use malachite_blocksync as blocksync;
use malachite_common::Context;

use crate::behaviour::{Behaviour, NetworkEvent};

impl<Ctx, N> NetworkBehaviour for Behaviour<Ctx, N>
where
    Ctx: Context,
    N: blocksync::NetworkCodec<Ctx>,
    identify::Behaviour: NetworkBehaviour,
    ping::Behaviour: NetworkBehaviour,
    Either<gossipsub::Behaviour, broadcast::Behaviour>: NetworkBehaviour,
    blocksync::Behaviour<Ctx, N>: NetworkBehaviour,
    NetworkEvent<Ctx>: From<<identify::Behaviour as NetworkBehaviour>::ToSwarm>,
    NetworkEvent<Ctx>: From<<ping::Behaviour as NetworkBehaviour>::ToSwarm>,
    NetworkEvent<Ctx>:
        From<<Either<gossipsub::Behaviour, broadcast::Behaviour> as NetworkBehaviour>::ToSwarm>,
    NetworkEvent<Ctx>: From<<blocksync::Behaviour<Ctx, N> as NetworkBehaviour>::ToSwarm>,
{
    type ConnectionHandler = ConnectionHandlerSelect<
        ConnectionHandlerSelect<
            ConnectionHandlerSelect<THandler<identify::Behaviour>, THandler<ping::Behaviour>>,
            THandler<Either<gossipsub::Behaviour, broadcast::Behaviour>>,
        >,
        THandler<blocksync::Behaviour<Ctx, N>>,
    >;

    type ToSwarm = NetworkEvent<Ctx>;

    #[allow(clippy::needless_question_mark)]
    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        NetworkBehaviour::handle_pending_inbound_connection(
            &mut self.identify,
            connection_id,
            local_addr,
            remote_addr,
        )?;

        NetworkBehaviour::handle_pending_inbound_connection(
            &mut self.ping,
            connection_id,
            local_addr,
            remote_addr,
        )?;

        NetworkBehaviour::handle_pending_inbound_connection(
            &mut self.pubsub,
            connection_id,
            local_addr,
            remote_addr,
        )?;

        NetworkBehaviour::handle_pending_inbound_connection(
            &mut self.blocksync,
            connection_id,
            local_addr,
            remote_addr,
        )?;

        Ok(())
    }

    #[allow(clippy::needless_question_mark)]
    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        local_addr: &Multiaddr,
        remote_addr: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(ConnectionHandler::select(
            ConnectionHandler::select(
                ConnectionHandler::select(
                    self.identify.handle_established_inbound_connection(
                        connection_id,
                        peer,
                        local_addr,
                        remote_addr,
                    )?,
                    self.ping.handle_established_inbound_connection(
                        connection_id,
                        peer,
                        local_addr,
                        remote_addr,
                    )?,
                ),
                self.pubsub.handle_established_inbound_connection(
                    connection_id,
                    peer,
                    local_addr,
                    remote_addr,
                )?,
            ),
            self.blocksync.handle_established_inbound_connection(
                connection_id,
                peer,
                local_addr,
                remote_addr,
            )?,
        ))
    }

    #[allow(clippy::needless_question_mark)]
    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        maybe_peer: Option<PeerId>,
        addresses: &[Multiaddr],
        effective_role: Endpoint,
    ) -> Result<::std::vec::Vec<Multiaddr>, ConnectionDenied> {
        let mut combined_addresses = Vec::new();

        combined_addresses.extend(NetworkBehaviour::handle_pending_outbound_connection(
            &mut self.identify,
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )?);

        combined_addresses.extend(NetworkBehaviour::handle_pending_outbound_connection(
            &mut self.ping,
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )?);

        combined_addresses.extend(NetworkBehaviour::handle_pending_outbound_connection(
            &mut self.pubsub,
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )?);

        combined_addresses.extend(NetworkBehaviour::handle_pending_outbound_connection(
            &mut self.blocksync,
            connection_id,
            maybe_peer,
            addresses,
            effective_role,
        )?);

        Ok(combined_addresses)
    }

    #[allow(clippy::needless_question_mark)]
    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        addr: &Multiaddr,
        role_override: Endpoint,
        port_use: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(ConnectionHandler::select(
            ConnectionHandler::select(
                ConnectionHandler::select(
                    self.identify.handle_established_outbound_connection(
                        connection_id,
                        peer,
                        addr,
                        role_override,
                        port_use,
                    )?,
                    self.ping.handle_established_outbound_connection(
                        connection_id,
                        peer,
                        addr,
                        role_override,
                        port_use,
                    )?,
                ),
                self.pubsub.handle_established_outbound_connection(
                    connection_id,
                    peer,
                    addr,
                    role_override,
                    port_use,
                )?,
            ),
            self.blocksync.handle_established_outbound_connection(
                connection_id,
                peer,
                addr,
                role_override,
                port_use,
            )?,
        ))
    }

    fn on_connection_handler_event(
        &mut self,
        peer_id: PeerId,
        connection_id: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {
            Either::Left(Either::Left(Either::Left(ev))) => {
                NetworkBehaviour::on_connection_handler_event(
                    &mut self.identify,
                    peer_id,
                    connection_id,
                    ev,
                )
            }
            Either::Left(Either::Left(Either::Right(ev))) => {
                NetworkBehaviour::on_connection_handler_event(
                    &mut self.ping,
                    peer_id,
                    connection_id,
                    ev,
                )
            }
            Either::Left(Either::Right(ev)) => NetworkBehaviour::on_connection_handler_event(
                &mut self.pubsub,
                peer_id,
                connection_id,
                ev,
            ),
            Either::Right(ev) => NetworkBehaviour::on_connection_handler_event(
                &mut self.blocksync,
                peer_id,
                connection_id,
                ev,
            ),
        }
    }
    fn poll(
        &mut self,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        match NetworkBehaviour::poll(&mut self.identify, cx) {
            std::task::Poll::Ready(e) => {
                return std::task::Poll::Ready(
                    e.map_out(|e| e.into())
                        .map_in(|event| Either::Left(Either::Left(Either::Left(event)))),
                );
            }
            std::task::Poll::Pending => {}
        }

        match NetworkBehaviour::poll(&mut self.ping, cx) {
            std::task::Poll::Ready(e) => {
                return std::task::Poll::Ready(
                    e.map_out(|e| e.into())
                        .map_in(|event| Either::Left(Either::Left(Either::Right(event)))),
                );
            }
            std::task::Poll::Pending => {}
        }

        match NetworkBehaviour::poll(&mut self.pubsub, cx) {
            std::task::Poll::Ready(e) => {
                return std::task::Poll::Ready(
                    e.map_out(|e| e.into())
                        .map_in(|event| Either::Left(Either::Right(event))),
                );
            }
            std::task::Poll::Pending => {}
        }

        match NetworkBehaviour::poll(&mut self.blocksync, cx) {
            std::task::Poll::Ready(e) => {
                return std::task::Poll::Ready(e.map_out(|e| e.into()).map_in(Either::Right));
            }
            std::task::Poll::Pending => {}
        }

        std::task::Poll::Pending
    }

    fn on_swarm_event(&mut self, event: FromSwarm) {
        self.identify.on_swarm_event(event);
        self.ping.on_swarm_event(event);
        self.pubsub.on_swarm_event(event);
        self.blocksync.on_swarm_event(event);
    }
}
