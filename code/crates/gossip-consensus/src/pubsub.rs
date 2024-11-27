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
            for channel in channels {
                swarm
                    .behaviour_mut()
                    .gossipsub
                    .subscribe(&channel.to_gossipsub_topic())?;
            }
        }
        PubSubProtocol::Broadcast => {
            for channel in channels {
                swarm
                    .behaviour_mut()
                    .broadcast
                    .subscribe(channel.to_broadcast_topic());
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
            swarm
                .behaviour_mut()
                .gossipsub
                .publish(channel.to_gossipsub_topic(), data)?;
        }
        PubSubProtocol::Broadcast => {
            swarm
                .behaviour_mut()
                .broadcast
                .broadcast(&channel.to_broadcast_topic(), data);
        }
    }

    Ok(())
}
