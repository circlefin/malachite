use bytes::Bytes;
use prost::Message;

use malachitebft_app::engine::util::streaming::{StreamContent, StreamId, StreamMessage};
use malachitebft_codec::Codec;
use malachitebft_core_consensus::{ProposedValue, SignedConsensusMsg};
use malachitebft_core_types::{
    CommitCertificate, CommitSignature, PolkaCertificate, PolkaSignature, Round, SignedExtension,
    SignedProposal, SignedVote, Validity, VoteSet,
};
use malachitebft_proto::{Error as ProtoError, Protobuf};
use malachitebft_signing_ed25519::Signature;
use malachitebft_sync::{self as sync, PeerId};

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

impl Codec<Signature> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<Signature, Self::Error> {
        let proto = proto::Signature::decode(bytes.as_ref())?;
        decode_signature(proto)
    }

    fn encode(&self, msg: &Signature) -> Result<Bytes, Self::Error> {
        Ok(Bytes::from(
            proto::Signature {
                bytes: Bytes::copy_from_slice(msg.to_bytes().as_ref()),
            }
            .encode_to_vec(),
        ))
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
            proto::stream_message::Content::Fin(_) => StreamContent::Fin,
        };

        Ok(StreamMessage {
            stream_id: StreamId::new(proto.stream_id),
            sequence: proto.sequence,
            content,
        })
    }

    fn encode(&self, msg: &StreamMessage<ProposalPart>) -> Result<Bytes, Self::Error> {
        let proto = proto::StreamMessage {
            stream_id: msg.stream_id.to_bytes(),
            sequence: msg.sequence,
            content: match &msg.content {
                StreamContent::Data(data) => {
                    Some(proto::stream_message::Content::Data(data.to_bytes()?))
                }
                StreamContent::Fin => Some(proto::stream_message::Content::Fin(true)),
            },
        };

        Ok(Bytes::from(proto.encode_to_vec()))
    }
}

impl Codec<ProposedValue<TestContext>> for ProtobufCodec {
    type Error = ProtoError;

    fn decode(&self, bytes: Bytes) -> Result<ProposedValue<TestContext>, Self::Error> {
        let proto = proto::ProposedValue::decode(bytes.as_ref())?;

        let proposer = proto
            .proposer
            .ok_or_else(|| ProtoError::missing_field::<proto::ProposedValue>("proposer"))?;

        let value = proto
            .value
            .ok_or_else(|| ProtoError::missing_field::<proto::ProposedValue>("value"))?;

        Ok(ProposedValue {
            height: Height::new(proto.height),
            round: Round::new(proto.round),
            valid_round: proto.valid_round.map(Round::new).unwrap_or(Round::Nil),
            proposer: Address::from_proto(proposer)?,
            value: Value::from_proto(value)?,
            validity: Validity::from_bool(proto.validity),
        })
    }

    fn encode(&self, msg: &ProposedValue<TestContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::ProposedValue {
            height: msg.height.as_u64(),
            round: msg.round.as_u32().unwrap(),
            valid_round: msg.valid_round.as_u32(),
            proposer: Some(msg.proposer.to_proto()?),
            value: Some(msg.value.to_proto()?),
            validity: msg.validity.to_bool(),
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
            tip_height: Height::new(proto.height),
            history_min_height: Height::new(proto.earliest_height),
        })
    }

    fn encode(&self, msg: &sync::Status<TestContext>) -> Result<Bytes, Self::Error> {
        let proto = proto::Status {
            peer_id: Some(proto::PeerId {
                id: Bytes::from(msg.peer_id.to_bytes()),
            }),
            height: msg.tip_height.as_u64(),
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

pub fn decode_sync_response(
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
                vote_set_response
                    .polka_certificates
                    .into_iter()
                    .map(decode_polka_certificate)
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
    };
    Ok(response)
}

pub fn encode_sync_response(
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
                    polka_certificates: vote_set_response
                        .polka_certificates
                        .iter()
                        .map(encode_polka_certificate)
                        .collect::<Result<Vec<_>, _>>()?,
                },
            )),
        },
    };

    Ok(proto)
}

pub fn encode_synced_value(
    synced_value: &sync::RawDecidedValue<TestContext>,
) -> Result<proto::SyncedValue, ProtoError> {
    Ok(proto::SyncedValue {
        value_bytes: synced_value.value_bytes.clone(),
        certificate: Some(encode_commit_certificate(&synced_value.certificate)?),
    })
}

pub fn decode_synced_value(
    proto: proto::SyncedValue,
) -> Result<sync::RawDecidedValue<TestContext>, ProtoError> {
    let certificate = proto
        .certificate
        .ok_or_else(|| ProtoError::missing_field::<proto::SyncedValue>("certificate"))?;

    Ok(sync::RawDecidedValue {
        value_bytes: proto.value_bytes,
        certificate: decode_commit_certificate(certificate)?,
    })
}

pub(crate) fn encode_polka_certificate(
    polka_certificate: &PolkaCertificate<TestContext>,
) -> Result<proto::PolkaCertificate, ProtoError> {
    Ok(proto::PolkaCertificate {
        height: polka_certificate.height.as_u64(),
        round: polka_certificate
            .round
            .as_u32()
            .expect("round should not be nil"),
        value_id: Some(polka_certificate.value_id.to_proto()?),
        signatures: polka_certificate
            .polka_signatures
            .iter()
            .map(|sig| -> Result<proto::PolkaSignature, ProtoError> {
                let address = sig.address.to_proto()?;
                let signature = encode_signature(&sig.signature);
                Ok(proto::PolkaSignature {
                    validator_address: Some(address),
                    signature: Some(signature),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn decode_polka_certificate(
    certificate: proto::PolkaCertificate,
) -> Result<PolkaCertificate<TestContext>, ProtoError> {
    let value_id = certificate
        .value_id
        .ok_or_else(|| ProtoError::missing_field::<proto::PolkaCertificate>("value_id"))
        .and_then(ValueId::from_proto)?;

    Ok(PolkaCertificate {
        height: Height::new(certificate.height),
        round: Round::new(certificate.round),
        value_id,
        polka_signatures: certificate
            .signatures
            .into_iter()
            .map(|sig| -> Result<PolkaSignature<TestContext>, ProtoError> {
                let address = sig.validator_address.ok_or_else(|| {
                    ProtoError::missing_field::<proto::PolkaCertificate>("validator_address")
                })?;
                let signature = sig.signature.ok_or_else(|| {
                    ProtoError::missing_field::<proto::PolkaCertificate>("signature")
                })?;
                let signature = decode_signature(signature)?;
                let address = Address::from_proto(address)?;
                Ok(PolkaSignature::new(address, signature))
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn decode_commit_certificate(
    certificate: proto::CommitCertificate,
) -> Result<CommitCertificate<TestContext>, ProtoError> {
    let value_id = certificate
        .value_id
        .ok_or_else(|| ProtoError::missing_field::<proto::CommitCertificate>("value_id"))
        .and_then(ValueId::from_proto)?;

    let commit_signatures = certificate
        .signatures
        .into_iter()
        .map(|sig| -> Result<CommitSignature<TestContext>, ProtoError> {
            let address = sig.validator_address.ok_or_else(|| {
                ProtoError::missing_field::<proto::CommitCertificate>("validator_address")
            })?;
            let signature = sig.signature.ok_or_else(|| {
                ProtoError::missing_field::<proto::CommitCertificate>("signature")
            })?;
            let signature = decode_signature(signature)?;
            let address = Address::from_proto(address)?;
            Ok(CommitSignature::new(address, signature))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let certificate = CommitCertificate {
        height: Height::new(certificate.height),
        round: Round::new(certificate.round),
        value_id,
        commit_signatures,
    };

    Ok(certificate)
}

pub fn encode_commit_certificate(
    certificate: &CommitCertificate<TestContext>,
) -> Result<proto::CommitCertificate, ProtoError> {
    Ok(proto::CommitCertificate {
        height: certificate.height.as_u64(),
        round: certificate.round.as_u32().expect("round should not be nil"),
        value_id: Some(certificate.value_id.to_proto()?),
        signatures: certificate
            .commit_signatures
            .iter()
            .map(|sig| -> Result<proto::CommitSignature, ProtoError> {
                let address = sig.address.to_proto()?;
                let signature = encode_signature(&sig.signature);
                Ok(proto::CommitSignature {
                    validator_address: Some(address),
                    signature: Some(signature),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn decode_extension(ext: proto::Extension) -> Result<SignedExtension<TestContext>, ProtoError> {
    let signature = ext
        .signature
        .ok_or_else(|| ProtoError::missing_field::<proto::Extension>("signature"))
        .and_then(decode_signature)?;

    Ok(SignedExtension::new(ext.data, signature))
}

pub fn encode_extension(
    ext: &SignedExtension<TestContext>,
) -> Result<proto::Extension, ProtoError> {
    Ok(proto::Extension {
        data: ext.message.clone(),
        signature: Some(encode_signature(&ext.signature)),
    })
}

pub fn encode_vote_set(vote_set: &VoteSet<TestContext>) -> Result<proto::VoteSet, ProtoError> {
    Ok(proto::VoteSet {
        signed_votes: vote_set
            .votes
            .iter()
            .map(encode_vote)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn encode_vote(vote: &SignedVote<TestContext>) -> Result<proto::SignedMessage, ProtoError> {
    Ok(proto::SignedMessage {
        message: Some(proto::signed_message::Message::Vote(
            vote.message.to_proto()?,
        )),
        signature: Some(encode_signature(&vote.signature)),
    })
}

pub fn decode_vote_set(vote_set: proto::VoteSet) -> Result<VoteSet<TestContext>, ProtoError> {
    Ok(VoteSet {
        votes: vote_set
            .signed_votes
            .into_iter()
            .map(decode_vote)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub fn decode_vote(msg: proto::SignedMessage) -> Result<SignedVote<TestContext>, ProtoError> {
    let signature = msg
        .signature
        .ok_or_else(|| ProtoError::missing_field::<proto::SignedMessage>("signature"))?;

    let vote = match msg.message {
        Some(proto::signed_message::Message::Vote(v)) => Ok(v),
        _ => Err(ProtoError::Other(
            "Invalid message type: not a vote".to_string(),
        )),
    }?;

    let signature = decode_signature(signature)?;
    let vote = Vote::from_proto(vote)?;
    Ok(SignedVote::new(vote, signature))
}

pub fn encode_signature(signature: &Signature) -> proto::Signature {
    proto::Signature {
        bytes: Bytes::copy_from_slice(signature.to_bytes().as_ref()),
    }
}

pub fn decode_signature(signature: proto::Signature) -> Result<Signature, ProtoError> {
    let bytes = <[u8; 64]>::try_from(signature.bytes.as_ref())
        .map_err(|_| ProtoError::Other("Invalid signature length".to_string()))?;
    Ok(Signature::from_bytes(bytes))
}
