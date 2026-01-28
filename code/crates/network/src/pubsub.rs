use bytes::Bytes;
use libp2p::swarm;

use crate::behaviour::Behaviour;
use crate::{Channel, ChannelNames, PeerIdExt, PubSubProtocol};

pub fn subscribe(
    swarm: &mut swarm::Swarm<Behaviour>,
    protocol: PubSubProtocol,
    channels: &[Channel],
    channel_names: ChannelNames,
) -> Result<(), eyre::Report> {
    match protocol {
        PubSubProtocol::GossipSub => {
            if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
                for channel in channels {
                    gossipsub.subscribe(&channel.to_gossipsub_topic(channel_names))?;
                }
            } else {
                return Err(eyre::eyre!("GossipSub not enabled"));
            }
        }
        PubSubProtocol::Broadcast => {
            if let Some(broadcast) = swarm.behaviour_mut().broadcast.as_mut() {
                for channel in channels {
                    broadcast.subscribe(channel.to_broadcast_topic(channel_names));
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
    channel_names: ChannelNames,
    data: Bytes,
) -> Result<(), eyre::Report> {
    match protocol {
        PubSubProtocol::GossipSub => {
            if let Some(gossipsub) = swarm.behaviour_mut().gossipsub.as_mut() {
                gossipsub.publish(channel.to_gossipsub_topic(channel_names), data)?;
            } else {
                return Err(eyre::eyre!("GossipSub not enabled"));
            }
        }
        PubSubProtocol::Broadcast => {
            if let Some(broadcast) = swarm.behaviour_mut().broadcast.as_mut() {
                broadcast.broadcast(&channel.to_broadcast_topic(channel_names), data);
            } else {
                return Err(eyre::eyre!("Broadcast not enabled"));
            }
        }
    }

    Ok(())
}

/// Get the mesh peers for a specific channel
pub fn get_mesh_peers(
    swarm: &swarm::Swarm<Behaviour>,
    channel: Channel,
    channel_names: ChannelNames,
) -> Vec<crate::PeerId> {
    if let Some(gossipsub) = swarm.behaviour().gossipsub.as_ref() {
        let topic = channel.to_gossipsub_topic(channel_names);
        let topic_hash = topic.hash();
        gossipsub
            .mesh_peers(&topic_hash)
            .map(crate::PeerId::from_libp2p)
            .collect()
    } else {
        Vec::new()
    }
}
