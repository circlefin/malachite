use bytes::Bytes;
use prost::Message;

use malachite_app::streaming::{StreamContent, StreamMessage};
use malachite_codec::Codec;
use malachite_core_consensus::SignedConsensusMsg;
use malachite_core_types::{
    AggregatedSignature, CommitCertificate, CommitSignature, Extension, Round, SignedExtension,
    SignedProposal, SignedVote, VoteSet,
};
use malachite_proto::{Error as ProtoError, Protobuf};
use malachite_signing_ed25519::Signature;
use malachite_sync::{self as sync, PeerId};

use crate::proto;
use crate::{Address, Height, Proposal, ProposalPart, TestContext, Value, ValueId, Vote};

#[derive(Copy, Clone, Debug)]
pub struct ProtobufCodec;

impl Codec<Value> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<Value, Self::Error> {
        Protobuf::from_bytes(&bytes)
    }

    fn encode(&self, msg: &Value) -> Result<Bytes, Self::Error> {
        Protobuf::to_bytes(msg)
    }
}

impl Codec<ProposalPart> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<ProposalPart, Self::Error> {
        Protobuf::from_bytes(&bytes)
    }

    fn encode(&self, msg: &ProposalPart) -> Result<Bytes, Self::Error> {
        Protobuf::to_bytes(msg)
    }
}

impl Codec<SignedConsensusMsg<TestContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<TestContext>, Self::Error> {
        let proto = proto::SignedMessage::decode(bytes.as_ref())?;

        let signature = proto
            .signature
            .ok_or_else(|| ProtoError::missing_field::<proto::SignedMessage>("signature"))
            .and_then(decode_signature)?;

        let proto_message = proto
            .message
            .ok_or_else(|| ProtoError::missing_field::<proto::SignedMessage>("message"))?;

        match proto_message {
            proto::signed_message::Message::Proposal(proto) => {
                let proposal = Proposal::from_proto(proto)?;
                Ok(SignedConsensusMsg::Proposal(SignedProposal::new(
                    proposal, signature,
                )))
            }
            proto::signed_message::Message::Vote(vote) => {
                let vote = Vote::from_proto(vote)?;
                Ok(SignedConsensusMsg::Vote(SignedVote::new(vote, signature)))
            }
        }
    }

    fn encode(&self, msg: &SignedConsensusMsg<TestContext>) -> Result<Bytes, Self::Error> {
        match msg {
            SignedConsensusMsg::Vote(vote) => {
                let proto = proto::SignedMessage {
                    message: Some(proto::signed_message::Message::Vote(
                        vote.message.to_proto()?,
                    )),
                    signature: Some(encode_signature(&vote.signature)),
                };
                Ok(Bytes::from(proto.encode_to_vec()))
            }
            SignedConsensusMsg::Proposal(proposal) => {
                let proto = proto::SignedMessage {
                    message: Some(proto::signed_message::Message::Proposal(
                        proposal.message.to_proto()?,
                    )),
                    signature: Some(encode_signature(&proposal.signature)),
                };
                Ok(Bytes::from(proto.encode_to_vec()))
            }
        }
    }
}

impl Codec<StreamMessage<ProposalPart>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<ProposalPart>, Self::Error> {
        let proto = proto::StreamMessage::decode(bytes.as_ref())?;

        let proto_content = proto
            .content
            .ok_or_else(|| ProtoError::missing_field::<proto::StreamMessage>("content"))?;

        let content = match proto_content {
            proto::stream_message::Content::Data(data) => {
                StreamContent::Data(ProposalPart::from_bytes(&data)?)
            }
            proto::stream_message::Content::Fin(end) => StreamContent::Fin(end),
        };

        Ok(StreamMessage {
            stream_id: proto.stream_id,
            sequence: proto.sequence,
            content,
        })
    }

    fn encode(&self, msg: &StreamMessage<ProposalPart>) -> Result<Bytes, Self::Error> {
        let proto = proto::StreamMessage {
            stream_id: msg.stream_id,
            sequence: msg.sequence,
            content: match &msg.content {
                StreamContent::Data(data) => {
                    Some(proto::stream_message::Content::Data(data.to_bytes()?))
                }
                StreamContent::Fin(end) => Some(proto::stream_message::Content::Fin(*end)),
            },
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<sync::Status<TestContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<sync::Status<TestContext>, Self::Error> {
        let proto = proto::Status::decode(bytes.as_ref())?;

        let proto_peer_id = proto
            .peer_id
            .ok_or_else(|| ProtoError::missing_field::<proto::Status>("peer_id"))?;

        Ok(sync::Status {
            peer_id: PeerId::from_bytes(proto_peer_id.id.as_ref()).unwrap(),
            height: Height::new(proto.height),
            history_min_height: Height::new(proto.earliest_height),
        })
    }

    fn encode(&self, msg: &sync::Status<TestContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::Status {
            peer_id: Some(proto::PeerId {
                id: Bytes::from(msg.peer_id.to_bytes()),
            }),
            height: msg.height.as_u64(),
            earliest_height: msg.history_min_height.as_u64(),
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<sync::Request<TestContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<sync::Request<TestContext>, Self::Error> {
        let proto = proto::SyncRequest::decode(bytes.as_ref())?;
        let request = proto
            .request
            .ok_or_else(|| ProtoError::missing_field::<proto::SyncRequest>("request"))?;

        match request {
            proto::sync_request::Request::ValueRequest(req) => Ok(sync::Request::ValueRequest(
                sync::ValueRequest::new(Height::new(req.height)),
            )),
            proto::sync_request::Request::VoteSetRequest(req) => Ok(sync::Request::VoteSetRequest(
                sync::VoteSetRequest::new(Height::new(req.height), Round::new(req.round)),
            )),
        }
    }

    fn encode(&self, msg: &sync::Request<TestContext>) -> Result<Bytes, Self::Error> {
        let proto = match msg {
            sync::Request::ValueRequest(req) => proto::SyncRequest {
                request: Some(proto::sync_request::Request::ValueRequest(
                    proto::ValueRequest {
                        height: req.height.as_u64(),
                    },
                )),
            },
            sync::Request::VoteSetRequest(req) => proto::SyncRequest {
                request: Some(proto::sync_request::Request::VoteSetRequest(
                    proto::VoteSetRequest {
                        height: req.height.as_u64(),
                        round: req.round.as_u32().unwrap(),
                    },
                )),
            },
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<sync::Response<TestContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<sync::Response<TestContext>, Self::Error> {
        decode_sync_response(proto::SyncResponse::decode(bytes)?)
    }

    fn encode(&self, response: &sync::Response<TestContext>) -> Result<Bytes, Self::Error> {
        encode_sync_response(response).map(|proto| proto.encode_to_vec().into())
    }
}

fn decode_sync_response(
    proto_response: proto::SyncResponse,
) -> Result<sync::Response<TestContext>, ProtoError> {
    let response = proto_response
        .response
        .ok_or_else(|| ProtoError::missing_field::<proto::SyncResponse>("messages"))?;

    let response = match response {
        proto::sync_response::Response::ValueResponse(value_response) => {
            sync::Response::ValueResponse(sync::ValueResponse::new(
                Height::new(value_response.height),
                value_response.value.map(decode_synced_value).transpose()?,
            ))
        }
        proto::sync_response::Response::VoteSetResponse(vote_set_response) => {
            let height = Height::new(vote_set_response.height);
            let round = Round::new(vote_set_response.round);
            let vote_set = vote_set_response
                .vote_set
                .ok_or_else(|| ProtoError::missing_field::<proto::VoteSet>("vote_set"))?;

            sync::Response::VoteSetResponse(sync::VoteSetResponse::new(
                height,
                round,
                decode_vote_set(vote_set)?,
            ))
        }
    };
    Ok(response)
}

fn encode_sync_response(
    response: &sync::Response<TestContext>,
) -> Result<proto::SyncResponse, ProtoError> {
    let proto = match response {
        sync::Response::ValueResponse(value_response) => proto::SyncResponse {
            response: Some(proto::sync_response::Response::ValueResponse(
                proto::ValueResponse {
                    height: value_response.height.as_u64(),
                    value: value_response
                        .value
                        .as_ref()
                        .map(encode_synced_value)
                        .transpose()?,
                },
            )),
        },
        sync::Response::VoteSetResponse(vote_set_response) => proto::SyncResponse {
            response: Some(proto::sync_response::Response::VoteSetResponse(
                proto::VoteSetResponse {
                    height: vote_set_response.height.as_u64(),
                    round: vote_set_response
                        .round
                        .as_u32()
                        .expect("round should not be nil"),
                    vote_set: Some(encode_vote_set(&vote_set_response.vote_set)?),
                },
            )),
        },
    };

    Ok(proto)
}

fn encode_synced_value(
    synced_value: &sync::DecidedValue<TestContext>,
) -> Result<proto::SyncedValue, ProtoError> {
    Ok(proto::SyncedValue {
        value_bytes: synced_value.value_bytes.clone(),
        certificate: Some(encode_certificate(&synced_value.certificate)?),
    })
}

fn decode_synced_value(
    proto: proto::SyncedValue,
) -> Result<sync::DecidedValue<TestContext>, ProtoError> {
    let certificate = proto
        .certificate
        .ok_or_else(|| ProtoError::missing_field::<proto::SyncedValue>("certificate"))?;

    Ok(sync::DecidedValue {
        value_bytes: proto.value_bytes,
        certificate: decode_certificate(certificate)?,
    })
}

fn decode_certificate(
    certificate: proto::CommitCertificate,
) -> Result<CommitCertificate<TestContext>, ProtoError> {
    let value_id = certificate
        .value_id
        .ok_or_else(|| ProtoError::missing_field::<proto::CommitCertificate>("value_id"))
        .and_then(ValueId::from_proto)?;

    let aggregated_signature = certificate
        .aggregated_signature
        .ok_or_else(|| {
            ProtoError::missing_field::<proto::CommitCertificate>("aggregated_signature")
        })
        .and_then(decode_aggregated_signature)?;

    let certificate = CommitCertificate {
        height: Height::new(certificate.height),
        round: Round::new(certificate.round),
        value_id,
        aggregated_signature,
    };

    Ok(certificate)
}

fn encode_certificate(
    certificate: &CommitCertificate<TestContext>,
) -> Result<proto::CommitCertificate, ProtoError> {
    Ok(proto::CommitCertificate {
        height: certificate.height.as_u64(),
        round: certificate.round.as_u32().expect("round should not be nil"),
        value_id: Some(certificate.value_id.to_proto()?),
        aggregated_signature: Some(encode_aggregate_signature(
            &certificate.aggregated_signature,
        )?),
    })
}

fn decode_aggregated_signature(
    signature: proto::AggregatedSignature,
) -> Result<AggregatedSignature<TestContext>, ProtoError> {
    let signatures = signature
        .signatures
        .into_iter()
        .map(|s| {
            let signature = s
                .signature
                .ok_or_else(|| ProtoError::missing_field::<proto::CommitSignature>("signature"))
                .and_then(decode_signature)?;

            let address = s
                .validator_address
                .ok_or_else(|| {
                    ProtoError::missing_field::<proto::CommitSignature>("validator_address")
                })
                .and_then(Address::from_proto)?;

            let extension = s.extension.map(decode_extension).transpose()?;

            Ok(CommitSignature {
                address,
                signature,
                extension,
            })
        })
        .collect::<Result<Vec<_>, ProtoError>>()?;

    Ok(AggregatedSignature { signatures })
}

fn encode_aggregate_signature(
    aggregated_signature: &AggregatedSignature<TestContext>,
) -> Result<proto::AggregatedSignature, ProtoError> {
    let signatures = aggregated_signature
        .signatures
        .iter()
        .map(|s| {
            Ok(proto::CommitSignature {
                validator_address: Some(s.address.to_proto()?),
                signature: Some(encode_signature(&s.signature)),
                extension: s.extension.as_ref().map(encode_extension).transpose()?,
            })
        })
        .collect::<Result<_, ProtoError>>()?;

    Ok(proto::AggregatedSignature { signatures })
}

fn decode_extension(ext: proto::Extension) -> Result<SignedExtension<TestContext>, ProtoError> {
    let extension = Extension::from(ext.data);
    let signature = ext
        .signature
        .ok_or_else(|| ProtoError::missing_field::<proto::Extension>("signature"))
        .and_then(decode_signature)?;

    Ok(SignedExtension::new(extension, signature))
}

fn encode_extension(ext: &SignedExtension<TestContext>) -> Result<proto::Extension, ProtoError> {
    Ok(proto::Extension {
        data: ext.message.data.clone(),
        signature: Some(encode_signature(&ext.signature)),
    })
}

fn encode_vote_set(vote_set: &VoteSet<TestContext>) -> Result<proto::VoteSet, ProtoError> {
    Ok(proto::VoteSet {
        signed_votes: vote_set
            .votes
            .iter()
            .map(encode_vote)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn encode_vote(vote: &SignedVote<TestContext>) -> Result<proto::SignedMessage, ProtoError> {
    Ok(proto::SignedMessage {
        message: Some(proto::signed_message::Message::Vote(
            vote.message.to_proto()?,
        )),
        signature: Some(encode_signature(&vote.signature)),
    })
}

fn decode_vote_set(vote_set: proto::VoteSet) -> Result<VoteSet<TestContext>, ProtoError> {
    Ok(VoteSet {
        votes: vote_set
            .signed_votes
            .into_iter()
            .filter_map(decode_vote)
            .collect(),
    })
}

fn decode_vote(msg: proto::SignedMessage) -> Option<SignedVote<TestContext>> {
    let signature = msg.signature?;
    let vote = match msg.message {
        Some(proto::signed_message::Message::Vote(v)) => Some(v),
        _ => None,
    }?;

    let signature = decode_signature(signature).ok()?;
    let vote = Vote::from_proto(vote).ok()?;
    Some(SignedVote::new(vote, signature))
}

pub(crate) fn encode_signature(signature: &Signature) -> proto::Signature {
    proto::Signature {
        bytes: Bytes::copy_from_slice(signature.to_bytes().as_ref()),
    }
}

pub(crate) fn decode_signature(signature: proto::Signature) -> Result<Signature, ProtoError> {
    let bytes = <[u8; 64]>::try_from(signature.bytes.as_ref())
        .map_err(|_| ProtoError::Other("Invalid signature length".to_string()))?;
    Ok(Signature::from_bytes(bytes))
}
