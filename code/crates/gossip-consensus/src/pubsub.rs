use bytes::Bytes;
use either::Either;
use libp2p::swarm;

use crate::behaviour::Behaviour;
use crate::Channel;

pub fn subscribe(
    swarm: &mut swarm::Swarm<Behaviour>,
    channels: &[Channel],
) -> Result<(), eyre::Report> {
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

pub fn publish(
    swarm: &mut swarm::Swarm<Behaviour>,
    channel: Channel,
    data: Bytes,
) -> Result<(), eyre::Report> {
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
