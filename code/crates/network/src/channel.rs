use core::fmt;

use libp2p::gossipsub;
use libp2p_broadcast as broadcast;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Channel {
    Consensus,
    Liveness,
    ProposalParts,
    Sync,
}

impl Channel {
    pub fn all() -> &'static [Channel] {
        &[
            Channel::Consensus,
            Channel::ProposalParts,
            Channel::Sync,
            Channel::Liveness,
        ]
    }

    pub fn consensus() -> &'static [Channel] {
        &[
            Channel::Consensus,
            Channel::ProposalParts,
            Channel::Liveness,
        ]
    }

    pub fn to_gossipsub_topic(self) -> gossipsub::IdentTopic {
        gossipsub::IdentTopic::new(self.as_str())
    }

    pub fn to_broadcast_topic(self) -> broadcast::Topic {
        broadcast::Topic::new(self.as_str().as_bytes())
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Channel::Consensus => "/consensus",
            Channel::ProposalParts => "/proposal_parts",
            Channel::Sync => "/sync",
            Channel::Liveness => "/liveness",
        }
    }

    pub fn has_gossipsub_topic(topic_hash: &gossipsub::TopicHash) -> bool {
        Self::all()
            .iter()
            .any(|channel| &channel.to_gossipsub_topic().hash() == topic_hash)
    }

    pub fn has_broadcast_topic(topic: &broadcast::Topic) -> bool {
        Self::all()
            .iter()
            .any(|channel| &channel.to_broadcast_topic() == topic)
    }

    pub fn from_gossipsub_topic_hash(topic: &gossipsub::TopicHash) -> Option<Self> {
        match topic.as_str() {
            "/consensus" => Some(Channel::Consensus),
            "/proposal_parts" => Some(Channel::ProposalParts),
            "/sync" => Some(Channel::Sync),
            "/liveness" => Some(Channel::Liveness),
            _ => None,
        }
    }

    pub fn from_broadcast_topic(topic: &broadcast::Topic) -> Option<Self> {
        match topic.as_ref() {
            b"/consensus" => Some(Channel::Consensus),
            b"/proposal_parts" => Some(Channel::ProposalParts),
            b"/sync" => Some(Channel::Sync),
            b"/liveness" => Some(Channel::Liveness),
            _ => None,
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_str().fmt(f)
    }
}
