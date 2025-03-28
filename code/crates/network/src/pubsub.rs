use bytes::Bytes;
use libp2p::swarm;

use crate::behaviour::Behaviour;
use crate::{Channel, PubSubProtocol};

pub fn subscribe(
    swarm: &mut swarm::Swarm<Behaviour>,
    protocol: PubSubProtocol,
    channels: &[Channel],
) -> Result<(), eyre::Report> {
    match protocol {
        PubSubProtocol::GossipSub => {
            if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
                for channel in channels {
                    gossipsub.subscribe(&channel.to_gossipsub_topic())?;
                }
            } else {
                return Err(eyre::eyre!("GossipSub not enabled"));
            }
        }
        PubSubProtocol::Broadcast => {
            if let Some(broadcast) = swarm.behaviour_mut().broadcast.as_mut() {
                for channel in channels {
                    broadcast.subscribe(channel.to_broadcast_topic());
                }
            } else {
                return Err(eyre::eyre!("Broadcast not enabled"));
            }
        }
    }

    Ok(())
}

pub fn publish(
    swarm: &mut swarm::Swarm<Behaviour>,
    protocol: PubSubProtocol,
    channel: Channel,
    data: Bytes,
) -> Result<(), eyre::Report> {
    match protocol {
        PubSubProtocol::GossipSub => {
            if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
                gossipsub.publish(channel.to_gossipsub_topic(), data)?;
            } else {
                return Err(eyre::eyre!("GossipSub not enabled"));
            }
        }
        PubSubProtocol::Broadcast => {
            if let Some(broadcast) = swarm.behaviour_mut().broadcast.as_mut() {
                broadcast.broadcast(&channel.to_broadcast_topic(), data);
            } else {
                return Err(eyre::eyre!("Broadcast not enabled"));
            }
        }
    }

    Ok(())
}
