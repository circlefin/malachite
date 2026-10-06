use bytes::Bytes;
use libp2p::{gossipsub, identity, swarm};
use libp2p_broadcast as broadcast;

use crate::behaviour::Behaviour;
use crate::{Channel, ChannelNames, PeerIdExt, PubSubMaxSizePerTopic, PubSubProtocol};

// This value matches the public-key inlining threshold in libp2p-gossipsub.
const MAX_INLINE_PUBLIC_KEY_LENGTH: usize = 42;

/// Whether `broadcast` itself is subscribed to `topic`.
pub(crate) fn broadcast_is_subscribed(
    broadcast: &broadcast::Behaviour,
    topic: &broadcast::Topic,
) -> bool {
    broadcast.subscribed().any(|subscribed| subscribed == topic)
}

/// Accept a broadcast topic only when it is a known channel *and* this node
/// subscribed the broadcast transport to it.
///
/// The transport delivers every inbound frame whose topic name maps to a
/// known channel. Default gossipsub deployments only subscribe broadcast to
/// Sync, so consensus / proposal-part / liveness frames on that path must
/// be dropped here.
pub(crate) fn accepted_broadcast_channel(
    swarm: &swarm::Swarm<Behaviour>,
    topic: &broadcast::Topic,
    channel_names: &ChannelNames,
) -> Option<Channel> {
    let channel = Channel::from_broadcast_topic(topic, channel_names)?;
    swarm
        .behaviour()
        .broadcast
        .as_ref()
        .is_some_and(|broadcast| broadcast_is_subscribed(broadcast, topic))
        .then_some(channel)
}

pub fn subscribe(
    swarm: &mut swarm::Swarm<Behaviour>,
    protocol: PubSubProtocol,
    channels: &[Channel],
    channel_names: &ChannelNames,
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

/// `libp2p-scatter` reports every `broadcast()` as success. Gossipsub refuses
/// a frame that exceeds its configured limit. Both paths must fail here.
/// Otherwise, `handle_ctrl_msg` logs an oversized send as published.
pub(crate) fn ensure_fits_max_size(len: usize, max_size: usize) -> Result<(), eyre::Report> {
    if len > max_size {
        return Err(eyre::eyre!(
            "Pubsub message of {len} bytes exceeds the configured maximum of {max_size} bytes"
        ));
    }
    Ok(())
}

/// Returns the encoded size of a signed GossipSub message with the given payload size.
pub(crate) fn signed_message_size(
    keypair: &identity::Keypair,
    channel: Channel,
    channel_names: &ChannelNames,
    payload_size: usize,
) -> Result<usize, identity::SigningError> {
    let public_key = keypair.public();
    let encoded_public_key = public_key.encode_protobuf();
    let key =
        (encoded_public_key.len() > MAX_INLINE_PUBLIC_KEY_LENGTH).then_some(encoded_public_key);
    let signature = keypair.sign(b"gossipsub-message-size")?;

    Ok(gossipsub::RawMessage {
        source: Some(public_key.to_peer_id()),
        data: vec![0; payload_size],
        sequence_number: Some(0),
        topic: channel.to_gossipsub_topic(channel_names).hash(),
        signature: Some(signature),
        key,
        validated: true,
    }
    .raw_protobuf_len())
}

pub(crate) fn publish_max_payload_size(
    protocol: PubSubProtocol,
    channel: Channel,
    global_max_size: usize,
    max_size_per_topic: PubSubMaxSizePerTopic,
) -> usize {
    if !protocol.is_gossipsub() {
        return global_max_size;
    }

    match channel {
        Channel::Consensus => max_size_per_topic.consensus,
        Channel::ProposalParts => max_size_per_topic.proposal_parts,
        Channel::Liveness => max_size_per_topic.liveness,
        Channel::Sync => None,
    }
    .unwrap_or(global_max_size)
}

pub fn publish(
    swarm: &mut swarm::Swarm<Behaviour>,
    protocol: PubSubProtocol,
    channel: Channel,
    channel_names: &ChannelNames,
    data: Bytes,
    max_size: usize,
) -> Result<(), eyre::Report> {
    ensure_fits_max_size(data.len(), max_size)?;

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
    channel_names: &ChannelNames,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_subscription_is_only_the_topics_we_subscribed() {
        let names = ChannelNames::default();
        let sync = Channel::Sync.to_broadcast_topic(&names);
        let consensus = Channel::Consensus.to_broadcast_topic(&names);
        let mut broadcast = broadcast::Behaviour::new(broadcast::Config::default());

        assert!(!broadcast_is_subscribed(&broadcast, &sync));
        assert!(!broadcast_is_subscribed(&broadcast, &consensus));

        broadcast.subscribe(sync);
        assert!(broadcast_is_subscribed(&broadcast, &sync));
        assert!(
            !broadcast_is_subscribed(&broadcast, &consensus),
            "a consensus frame on broadcast must not pass when only Sync is subscribed"
        );
    }

    #[test]
    fn pubsub_rejects_payloads_over_the_configured_limit() {
        assert!(ensure_fits_max_size(0, 4).is_ok());
        assert!(ensure_fits_max_size(4, 4).is_ok());
        let err = ensure_fits_max_size(5, 4).expect_err("over-limit payload");
        assert!(err.to_string().contains("exceeds the configured maximum"));
    }

    #[test]
    fn topic_payload_limits_only_apply_to_gossipsub_consensus_channels() {
        let per_topic = PubSubMaxSizePerTopic {
            proposal_parts: Some(128),
            ..Default::default()
        };

        assert_eq!(
            publish_max_payload_size(
                PubSubProtocol::GossipSub,
                Channel::ProposalParts,
                1024,
                per_topic
            ),
            128
        );
        assert_eq!(
            publish_max_payload_size(PubSubProtocol::GossipSub, Channel::Sync, 1024, per_topic),
            1024
        );
        assert_eq!(
            publish_max_payload_size(
                PubSubProtocol::Broadcast,
                Channel::ProposalParts,
                1024,
                per_topic
            ),
            1024
        );
    }

    #[test]
    fn signed_message_size_adds_the_gossipsub_envelope() {
        let keypair = identity::Keypair::generate_ed25519();
        let channel_names = ChannelNames::default();
        let payload_size = 1024;

        let encoded_size = signed_message_size(
            &keypair,
            Channel::ProposalParts,
            &channel_names,
            payload_size,
        )
        .unwrap();

        assert!(encoded_size > payload_size);
    }
}
