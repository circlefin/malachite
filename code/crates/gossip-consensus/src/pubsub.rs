use bytes::Bytes;
use either::Either;
use libp2p::swarm;
use malachite_blocksync::NetworkCodec;
use malachite_common::Context;

use crate::behaviour::Behaviour;
use crate::Channel;

pub fn subscribe<Ctx, N>(
    swarm: &mut swarm::Swarm<Behaviour<Ctx, N>>,
    channels: &[Channel],
) -> Result<(), eyre::Report>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    match &mut swarm.behaviour_mut().pubsub {
        Either::Left(gossipsub) => {
            for channel in channels {
                gossipsub.subscribe(&channel.to_gossipsub_topic())?;
            }
        }
        Either::Right(broadcast) => {
            for channel in channels {
                broadcast.subscribe(channel.to_broadcast_topic());
            }
        }
    }

    Ok(())
}

pub fn publish<Ctx, N>(
    swarm: &mut swarm::Swarm<Behaviour<Ctx, N>>,
    channel: Channel,
    data: Bytes,
) -> Result<(), eyre::Report>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    match &mut swarm.behaviour_mut().pubsub {
        Either::Left(gossipsub) => {
            gossipsub.publish(channel.to_gossipsub_topic(), data)?;
        }
        Either::Right(broadcast) => {
            broadcast.broadcast(&channel.to_broadcast_topic(), data);
        }
    }

    Ok(())
}
