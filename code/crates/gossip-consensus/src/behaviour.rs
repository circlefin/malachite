use std::time::Duration;

use either::Either;
use libp2p::{gossipsub, identify, ping};
use libp2p_broadcast as broadcast;

pub use libp2p::identity::Keypair;
pub use libp2p::{Multiaddr, PeerId};

use malachite_blocksync as blocksync;
use malachite_common::Context;
use malachite_metrics::Registry;

use crate::{PubSubProtocol, PROTOCOL};

const MAX_TRANSMIT_SIZE: usize = 4 * 1024 * 1024; // 4 MiB

#[derive(Debug)]
pub enum NetworkEvent<Ctx: Context> {
    Identify(identify::Event),
    Ping(ping::Event),
    GossipSub(gossipsub::Event),
    Broadcast(broadcast::Event),
    BlockSync(blocksync::Event<Ctx>),
}

impl<Ctx: Context> From<identify::Event> for NetworkEvent<Ctx> {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl<Ctx: Context> From<ping::Event> for NetworkEvent<Ctx> {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl<Ctx: Context> From<gossipsub::Event> for NetworkEvent<Ctx> {
    fn from(event: gossipsub::Event) -> Self {
        Self::GossipSub(event)
    }
}

impl<Ctx: Context> From<broadcast::Event> for NetworkEvent<Ctx> {
    fn from(event: broadcast::Event) -> Self {
        Self::Broadcast(event)
    }
}

impl<Ctx: Context> From<blocksync::Event<Ctx>> for NetworkEvent<Ctx> {
    fn from(event: blocksync::Event<Ctx>) -> Self {
        Self::BlockSync(event)
    }
}

impl<Ctx, A, B> From<Either<A, B>> for NetworkEvent<Ctx>
where
    Ctx: Context,
    A: Into<NetworkEvent<Ctx>>,
    B: Into<NetworkEvent<Ctx>>,
{
    fn from(event: Either<A, B>) -> Self {
        match event {
            Either::Left(event) => event.into(),
            Either::Right(event) => event.into(),
        }
    }
}

pub struct Behaviour<Ctx: Context, C: blocksync::NetworkCodec<Ctx>> {
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub pubsub: Either<gossipsub::Behaviour, broadcast::Behaviour>,
    pub blocksync: blocksync::Behaviour<Ctx, C>,
}

fn message_id(message: &gossipsub::Message) -> gossipsub::MessageId {
    use seahash::SeaHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = SeaHasher::new();
    message.hash(&mut hasher);
    gossipsub::MessageId::new(hasher.finish().to_be_bytes().as_slice())
}

fn gossipsub_config() -> gossipsub::Config {
    gossipsub::ConfigBuilder::default()
        .max_transmit_size(MAX_TRANSMIT_SIZE)
        .opportunistic_graft_ticks(3)
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .history_gossip(3)
        .history_length(5)
        .mesh_n_high(4)
        .mesh_n_low(1)
        .mesh_outbound_min(1)
        .mesh_n(3)
        .message_id_fn(message_id)
        .build()
        .unwrap()
}

impl<Ctx, C> Behaviour<Ctx, C>
where
    Ctx: Context,
    C: malachite_blocksync::NetworkCodec<Ctx>,
{
    pub fn new_with_metrics(
        tpe: PubSubProtocol,
        keypair: &Keypair,
        registry: &mut Registry,
    ) -> Self {
        let identify = identify::Behaviour::new(identify::Config::new(
            PROTOCOL.to_string(),
            keypair.public(),
        ));

        let ping = ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(5)));

        let pubsub = match tpe {
            PubSubProtocol::GossipSub => Either::Left(
                gossipsub::Behaviour::new_with_metrics(
                    gossipsub::MessageAuthenticity::Signed(keypair.clone()),
                    gossipsub_config(),
                    registry.sub_registry_with_prefix("gossipsub"),
                    Default::default(),
                )
                .unwrap(),
            ),
            PubSubProtocol::Broadcast => Either::Right(broadcast::Behaviour::new_with_metrics(
                broadcast::Config {
                    max_buf_size: MAX_TRANSMIT_SIZE,
                },
                registry.sub_registry_with_prefix("broadcast"),
            )),
        };

        let blocksync =
            blocksync::Behaviour::new_with_metrics(registry.sub_registry_with_prefix("blocksync"));

        Self {
            identify,
            ping,
            pubsub,
            blocksync,
        }
    }
}
