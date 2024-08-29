use malachite_gossip_consensus::Bytes;
use prost::Message;

use malachite_actors::util::codec::NetworkCodec;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_common::{SignedProposal, SignedVote};
use malachite_consensus::GossipMsg;
use malachite_proto::{Error as ProtoError, Protobuf};
use malachite_starknet_host::mock::context::MockContext;
use malachite_starknet_host::types::Vote;
use malachite_starknet_p2p_proto::consensus_message::Messages;
use malachite_starknet_p2p_proto::ConsensusMessage;
use malachite_starknet_p2p_types as p2p;

pub struct ProtobufCodec;

impl NetworkCodec<MockContext> for ProtobufCodec {
    type Error = ProtoError;

    fn decode_msg(bytes: Bytes) -> Result<GossipMsg<MockContext>, Self::Error> {
        let proto = ConsensusMessage::decode(bytes)?;

        let proto_signature = proto
            .signature
            .ok_or_else(|| ProtoError::missing_field::<ConsensusMessage>("signature"))?;

        let message = proto
            .messages
            .ok_or_else(|| ProtoError::missing_field::<ConsensusMessage>("messages"))?;

        let signature = p2p::Signature::from_proto(proto_signature)?;

        match message {
            Messages::Vote(v) => {
                Vote::from_proto(v).map(|v| GossipMsg::Vote(SignedVote::new(v, signature)))
            }
            Messages::Proposal(p) => p2p::Proposal::from_proto(p)
                .map(|p| GossipMsg::Proposal(SignedProposal::new(p, signature))),
        }
    }

    fn encode_msg(msg: GossipMsg<MockContext>) -> Result<Bytes, Self::Error> {
        let message = match msg {
            GossipMsg::Vote(v) => ConsensusMessage {
                messages: Some(Messages::Vote(v.to_proto()?)),
                signature: Some(v.signature.to_proto()?),
            },
            GossipMsg::Proposal(p) => ConsensusMessage {
                messages: Some(Messages::Proposal(p.to_proto()?)),
                signature: Some(p.signature.to_proto()?),
            },
        };

        Ok(Bytes::from(prost::Message::encode_to_vec(&message)))
    }

    fn decode_stream_msg<T>(bytes: Bytes) -> Result<StreamMessage<T>, Self::Error>
    where
        T: Protobuf,
    {
        let p2p_msg = p2p::StreamMessage::from_bytes(&bytes)?;
        Ok(StreamMessage {
            stream_id: p2p_msg.id,
            sequence: p2p_msg.sequence,
            content: match p2p_msg.content {
                p2p::StreamContent::Data(data) => StreamContent::Data(T::from_bytes(&data)?),
                p2p::StreamContent::Fin(fin) => StreamContent::Fin(fin),
            },
        })
    }

    fn encode_stream_msg<T>(msg: StreamMessage<T>) -> Result<Bytes, Self::Error>
    where
        T: Protobuf,
    {
        let p2p_msg = p2p::StreamMessage {
            id: msg.stream_id,
            sequence: msg.sequence,
            content: match msg.content {
                StreamContent::Data(data) => p2p::StreamContent::Data(data.to_bytes()?),
                StreamContent::Fin(fin) => p2p::StreamContent::Fin(fin),
            },
        };

        Ok(Bytes::from(p2p_msg.to_bytes()?))
    }
}
