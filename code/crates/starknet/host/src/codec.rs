use prost::Message;

use malachite_actors::util::codec::NetworkCodec;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_blocksync as blocksync;
use malachite_common::{
    AggregatedSignature, CommitCertificate, CommitSignature, Extension, Round, SignedExtension,
    SignedProposal, SignedVote,
};
use malachite_consensus::SignedConsensusMsg;
use malachite_gossip_consensus::Bytes;

use crate::proto::consensus_message::Messages;
use crate::proto::{self as proto, ConsensusMessage, Error as ProtoError, Protobuf};
use crate::types::MockContext;
use crate::types::{self as p2p, Address, BlockHash, Height, ProposalPart, Vote};

pub struct ProtobufCodec;

impl NetworkCodec<ProposalPart> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<ProposalPart, Self::Error> {
        ProposalPart::from_bytes(bytes.as_ref())
    }

    fn encode(&self, msg: ProposalPart) -> Result<Bytes, Self::Error> {
        msg.to_bytes()
    }
}

impl NetworkCodec<blocksync::Status<MockContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<blocksync::Status<MockContext>, Self::Error> {
        let status =
            proto::blocksync::Status::decode(bytes.as_ref()).map_err(ProtoError::Decode)?;

        let peer_id = status
            .peer_id
            .ok_or_else(|| ProtoError::missing_field::<proto::blocksync::Status>("peer_id"))?;

        Ok(blocksync::Status {
            peer_id: libp2p_identity::PeerId::from_bytes(&peer_id.id)
                .map_err(|e| ProtoError::Other(e.to_string()))?,
            height: Height::new(status.block_number, status.fork_id),
            earliest_block_height: Height::new(
                status.earliest_block_number,
                status.earliest_fork_id,
            ),
        })
    }

    fn encode(&self, status: blocksync::Status<MockContext>) -> Result<Bytes, Self::Error> {
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
}

impl NetworkCodec<blocksync::Request<MockContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<blocksync::Request<MockContext>, Self::Error> {
        let request = proto::blocksync::Request::decode(bytes).map_err(ProtoError::Decode)?;

        Ok(blocksync::Request {
            height: Height::new(request.block_number, request.fork_id),
        })
    }

    fn encode(&self, request: blocksync::Request<MockContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::blocksync::Request {
            block_number: request.height.block_number,
            fork_id: request.height.fork_id,
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl NetworkCodec<blocksync::Response<MockContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<blocksync::Response<MockContext>, Self::Error> {
        let response = proto::blocksync::Response::decode(bytes).map_err(ProtoError::Decode)?;

        Ok(blocksync::Response {
            height: Height::new(response.block_number, response.fork_id),
            block: response.block.map(decode_sync_block).transpose()?,
        })
    }

    fn encode(&self, response: blocksync::Response<MockContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::blocksync::Response {
            block_number: response.height.block_number,
            fork_id: response.height.fork_id,
            block: response.block.map(encode_synced_block).transpose()?,
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl NetworkCodec<SignedConsensusMsg<MockContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<MockContext>, Self::Error> {
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

    fn encode(&self, msg: SignedConsensusMsg<MockContext>) -> Result<Bytes, Self::Error> {
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
}

impl<T> NetworkCodec<StreamMessage<T>> for ProtobufCodec
where
    T: Protobuf,
{
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<T>, Self::Error> {
        let p2p_msg = p2p::StreamMessage::from_bytes(&bytes)?;
        Ok(StreamMessage {
            stream_id: p2p_msg.id,
            sequence: p2p_msg.sequence,
            content: match p2p_msg.content {
                p2p::StreamContent::Data(data) => {
                    StreamContent::Data(T::from_bytes(data.as_ref())?)
                }
                p2p::StreamContent::Fin(fin) => StreamContent::Fin(fin),
            },
        })
    }

    fn encode(&self, msg: StreamMessage<T>) -> Result<Bytes, Self::Error> {
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

pub(crate) fn encode_aggregate_signature(
    aggregated_signature: AggregatedSignature<MockContext>,
) -> Result<proto::AggregatedSignature, ProtoError> {
    let signatures = aggregated_signature
        .signatures
        .into_iter()
        .map(|s| {
            let validator_address = s.address.to_proto()?;
            let signature = s.signature.to_proto()?;

            Ok(proto::CommitSignature {
                validator_address: Some(validator_address),
                signature: Some(signature),
                extension: s
                    .extension
                    .map(|e| -> Result<_, ProtoError> {
                        Ok(proto::Extension {
                            data: e.message.data,
                            signature: Some(e.signature.to_proto()?),
                        })
                    })
                    .transpose()?,
            })
        })
        .collect::<Result<_, ProtoError>>()?;

    Ok(proto::AggregatedSignature { signatures })
}

pub(crate) fn encode_certificate(
    certificate: CommitCertificate<MockContext>,
) -> Result<proto::CommitCertificate, ProtoError> {
    Ok(proto::CommitCertificate {
        fork_id: certificate.height.fork_id,
        block_number: certificate.height.block_number,
        round: certificate.round.as_u32().expect("round should not be nil"),
        block_hash: Some(certificate.value_id.to_proto()?),
        aggregated_signature: Some(encode_aggregate_signature(
            certificate.aggregated_signature,
        )?),
    })
}

pub(crate) fn encode_synced_block(
    synced_block: blocksync::SyncedBlock<MockContext>,
) -> Result<proto::blocksync::SyncedBlock, ProtoError> {
    Ok(proto::blocksync::SyncedBlock {
        block_bytes: synced_block.block_bytes,
        certificate: Some(encode_certificate(synced_block.certificate)?),
    })
}

pub(crate) fn decode_aggregated_signature(
    signature: proto::AggregatedSignature,
) -> Result<AggregatedSignature<MockContext>, ProtoError> {
    let signatures = signature
        .signatures
        .into_iter()
        .map(|s| {
            let signature = s
                .signature
                .ok_or_else(|| ProtoError::missing_field::<proto::CommitSignature>("signature"))
                .and_then(p2p::Signature::from_proto)?;

            let address = s
                .validator_address
                .ok_or_else(|| {
                    ProtoError::missing_field::<proto::CommitSignature>("validator_address")
                })
                .and_then(Address::from_proto)?;

            let extension = s
                .extension
                .map(|e| -> Result<_, ProtoError> {
                    let extension = Extension::from(e.data);
                    let signature = e
                        .signature
                        .ok_or_else(|| ProtoError::missing_field::<proto::Extension>("signature"))
                        .and_then(p2p::Signature::from_proto)?;

                    Ok(SignedExtension::new(extension, signature))
                })
                .transpose()?;

            Ok(CommitSignature {
                address,
                signature,
                extension,
            })
        })
        .collect::<Result<Vec<_>, ProtoError>>()?;

    Ok(AggregatedSignature { signatures })
}

pub(crate) fn decode_certificate(
    certificate: proto::CommitCertificate,
) -> Result<CommitCertificate<MockContext>, ProtoError> {
    let value_id = if let Some(block_hash) = certificate.block_hash {
        BlockHash::from_proto(block_hash)?
    } else {
        return Err(ProtoError::missing_field::<proto::CommitCertificate>(
            "block_hash",
        ));
    };

    let aggregated_signature = if let Some(agg_sig) = certificate.aggregated_signature {
        decode_aggregated_signature(agg_sig)?
    } else {
        return Err(ProtoError::missing_field::<proto::CommitCertificate>(
            "aggregated_signature",
        ));
    };

    let certificate = CommitCertificate {
        height: Height::new(certificate.block_number, certificate.fork_id),
        round: Round::new(certificate.round),
        value_id,
        aggregated_signature,
    };

    Ok(certificate)
}

pub(crate) fn decode_sync_block(
    synced_block: proto::blocksync::SyncedBlock,
) -> Result<blocksync::SyncedBlock<MockContext>, ProtoError> {
    let certificate = if let Some(certificate) = synced_block.certificate {
        certificate
    } else {
        return Err(ProtoError::missing_field::<proto::blocksync::SyncedBlock>(
            "certificate",
        ));
    };

    Ok(blocksync::SyncedBlock {
        block_bytes: synced_block.block_bytes,
        certificate: decode_certificate(certificate)?,
    })
}
