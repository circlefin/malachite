use prost::Message;

use malachite_common::{SignedProposal, SignedProposalPart, SignedVote};
use malachite_gossip_consensus::{GossipMsg, NetworkCodec};
use malachite_proto::{Error as ProtoError, Protobuf};
use malachite_starknet_host::mock::context::MockContext;
use malachite_starknet_host::types::Vote;
use malachite_starknet_p2p_proto::consensus_message::Messages;
use malachite_starknet_p2p_proto::ConsensusMessage;
use malachite_starknet_p2p_types::{Proposal, ProposalPart, Signature};

pub struct ProtobufCodec;

impl NetworkCodec<MockContext> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: &[u8]) -> Result<GossipMsg<MockContext>, Self::Error> {
        let proto = ConsensusMessage::decode(bytes)?;

        let signature = Signature::from_proto(
            proto
                .signature
                .ok_or_else(|| ProtoError::missing_field::<ConsensusMessage>("signature"))?,
        )?;

        let message = proto
            .messages
            .ok_or_else(|| ProtoError::missing_field::<ConsensusMessage>("messages"))?;

        match message {
            Messages::Vote(v) => {
                Vote::from_proto(v).map(|v| GossipMsg::Vote(SignedVote::new(v, signature)))
            }
            Messages::Proposal(p) => Proposal::from_proto(p)
                .map(|p| GossipMsg::Proposal(SignedProposal::new(p, signature))),
            Messages::ProposalPart(pp) => ProposalPart::from_proto(pp)
                .map(|pp| GossipMsg::ProposalPart(SignedProposalPart::new(pp, signature))),
        }
    }

    fn encode(&self, msg: GossipMsg<MockContext>) -> Result<Vec<u8>, Self::Error> {
        let message = match msg {
            GossipMsg::Vote(v) => ConsensusMessage {
                messages: Some(Messages::Vote(v.to_proto()?)),
                signature: Some(v.signature.to_proto()?),
            },
            GossipMsg::Proposal(p) => ConsensusMessage {
                messages: Some(Messages::Proposal(p.to_proto()?)),
                signature: Some(p.signature.to_proto()?),
            },

            GossipMsg::ProposalPart(pp) => ConsensusMessage {
                messages: Some(Messages::ProposalPart(pp.to_proto()?)),
                signature: Some(pp.signature.to_proto()?),
            },
        };

        Ok(prost::Message::encode_to_vec(&message))
    }
}
