use prost::Message;

use blocksync::Status;
use malachite_actors::util::codec::NetworkCodec;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_common::{Certificate, SignedProposal, SignedVote};
use malachite_consensus::SignedConsensusMsg;
use malachite_gossip_consensus::Bytes;
use malachite_proto::{Error as ProtoError, Protobuf};
use malachite_starknet_p2p_types::{self as p2p, Height};

use malachite_starknet_p2p_proto::consensus_message::Messages;
use malachite_starknet_p2p_proto::ConsensusMessage;
use malachite_starknet_p2p_proto::{self as proto};

use malachite_blocksync as blocksync;

use crate::mock::context::MockContext;
use crate::types::{Proposal, Vote};

pub struct ProtobufCodec;

impl blocksync::NetworkCodec<MockContext> for ProtobufCodec {
    type Error = ProtoError;

    fn decode_status(bytes: Bytes) -> Result<Status<MockContext>, Self::Error> {
        let status =
            proto::blocksync::Status::decode(bytes.as_ref()).map_err(ProtoError::Decode)?;

        let peer_id = status
            .peer_id
            .ok_or_else(|| ProtoError::missing_field::<proto::blocksync::Status>("peer_id"))?;

        Ok(Status {
            peer_id: libp2p_identity::PeerId::from_bytes(&peer_id.id)
                .map_err(|e| ProtoError::Other(e.to_string()))?,
            height: Height::new(status.block_number, status.fork_id),
            earliest_block_height: Height::new(
                status.earliest_block_number,
                status.earliest_fork_id,
            ),
        })
    }

    fn encode_status(status: Status<MockContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::blocksync::Status {
            peer_id: Some(proto::PeerId {
                id: Bytes::from(status.peer_id.to_bytes()),
            }),
            block_number: status.height.block_number,
            fork_id: status.height.fork_id,
            earliest_block_number: status.earliest_block_height.block_number,
            earliest_fork_id: status.earliest_block_height.fork_id,
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }

    fn decode_request(bytes: Bytes) -> Result<blocksync::Request<MockContext>, Self::Error> {
        let request = proto::blocksync::Request::decode(bytes).map_err(ProtoError::Decode)?;

        Ok(blocksync::Request {
            height: Height::new(request.block_number, request.fork_id),
        })
    }

    fn encode_request(request: blocksync::Request<MockContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::blocksync::Request {
            block_number: request.height.block_number,
            fork_id: request.height.fork_id,
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }

    fn decode_response(bytes: Bytes) -> Result<blocksync::Response<MockContext>, Self::Error> {
        let response = proto::blocksync::Response::decode(bytes).map_err(ProtoError::Decode)?;

        Ok(blocksync::Response {
            height: Height::new(response.block_number, response.fork_id),
            block: response.block.map(decode_sync_block).transpose()?,
        })
    }

    fn encode_response(response: blocksync::Response<MockContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::blocksync::Response {
            block_number: response.height.block_number,
            fork_id: response.height.fork_id,
            block: response.block.map(encode_synced_block).transpose()?,
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl NetworkCodec<MockContext> for ProtobufCodec {
    fn decode_msg(bytes: Bytes) -> Result<SignedConsensusMsg<MockContext>, Self::Error> {
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
                Vote::from_proto(v).map(|v| SignedConsensusMsg::Vote(SignedVote::new(v, signature)))
            }
            Messages::Proposal(p) => p2p::Proposal::from_proto(p)
                .map(|p| SignedConsensusMsg::Proposal(SignedProposal::new(p, signature))),
        }
    }

    fn encode_msg(msg: SignedConsensusMsg<MockContext>) -> Result<Bytes, Self::Error> {
        let message = match msg {
            SignedConsensusMsg::Vote(v) => ConsensusMessage {
                messages: Some(Messages::Vote(v.to_proto()?)),
                signature: Some(v.signature.to_proto()?),
            },
            SignedConsensusMsg::Proposal(p) => ConsensusMessage {
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

        p2p_msg.to_bytes()
    }
}

pub(crate) fn encode_proposal(
    proposal: SignedProposal<MockContext>,
) -> Result<ConsensusMessage, ProtoError> {
    Ok(ConsensusMessage {
        messages: Some(Messages::Proposal(proposal.message.to_proto()?)),
        signature: Some(proposal.signature.to_proto()?),
    })
}

pub(crate) fn encode_vote(vote: SignedVote<MockContext>) -> Result<ConsensusMessage, ProtoError> {
    Ok(ConsensusMessage {
        messages: Some(Messages::Vote(vote.message.to_proto()?)),
        signature: Some(vote.signature.to_proto()?),
    })
}

pub(crate) fn encode_synced_block(
    synced_block: blocksync::SyncedBlock<MockContext>,
) -> Result<proto::blocksync::SyncedBlock, ProtoError> {
    let commits = synced_block
        .certificate
        .commits
        .into_iter()
        .map(encode_vote)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(proto::blocksync::SyncedBlock {
        proposal: Some(encode_proposal(synced_block.proposal)?),
        commits,
        block_bytes: synced_block.block_bytes,
    })
}

pub(crate) fn decode_proposal(msg: ConsensusMessage) -> Option<SignedProposal<MockContext>> {
    let signature = msg.signature?;
    let proposal = match msg.messages {
        Some(Messages::Proposal(p)) => Some(p),
        _ => None,
    }?;

    let signature = p2p::Signature::from_proto(signature).ok()?;
    let proposal = Proposal::from_proto(proposal).ok()?;
    Some(SignedProposal::new(proposal, signature))
}

pub(crate) fn decode_vote(msg: ConsensusMessage) -> Option<SignedVote<MockContext>> {
    let signature = msg.signature?;
    let vote = match msg.messages {
        Some(Messages::Vote(v)) => Some(v),
        _ => None,
    }?;

    let signature = p2p::Signature::from_proto(signature).ok()?;
    let vote = Vote::from_proto(vote).ok()?;
    Some(SignedVote::new(vote, signature))
}

pub(crate) fn decode_sync_block(
    synced_block: proto::blocksync::SyncedBlock,
) -> Result<blocksync::SyncedBlock<MockContext>, ProtoError> {
    let commits = synced_block
        .commits
        .into_iter()
        .filter_map(decode_vote)
        .collect();

    let certificate = Certificate::new(commits);

    Ok(blocksync::SyncedBlock {
        proposal: decode_proposal(synced_block.proposal.unwrap())
            .ok_or_else(|| ProtoError::missing_field::<ConsensusMessage>("proposal"))?,
        certificate,
        block_bytes: synced_block.block_bytes,
    })
}
