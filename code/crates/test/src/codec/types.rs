use crate::{Address, Height, Proposal, ProposalPart, RoundDef, TestContext, ValueId, Vote};
use bytes::Bytes;
use ed25519_consensus::Signature;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_consensus::SignedConsensusMsg;
use malachite_core_types::{
    AggregatedSignature, CommitCertificate, CommitSignature, Extension, Round, SignedExtension,
    SignedProposal, SignedVote, VoteSet,
};
use malachite_proto::Protobuf;
use malachite_sync::{
    DecidedValue, PeerId, Request, Response, Status, ValueRequest, ValueResponse, VoteSetRequest,
    VoteSetResponse,
};
use serde::{Deserialize, Serialize};

/// todo
#[derive(Serialize, Deserialize)]
pub struct RawSignedMessage {
    message: Bytes,
    signature: Signature,
}

#[derive(Serialize, Deserialize)]
pub enum RawSignedConsensusMsg {
    Vote(RawSignedMessage),
    Proposal(RawSignedMessage),
}

impl From<SignedConsensusMsg<TestContext>> for RawSignedConsensusMsg {
    fn from(value: SignedConsensusMsg<TestContext>) -> Self {
        match value {
            SignedConsensusMsg::Vote(vote) => Self::Vote(RawSignedMessage {
                message: vote.message.to_bytes(),
                signature: *vote.signature.inner(),
            }),
            SignedConsensusMsg::Proposal(proposal) => Self::Proposal(RawSignedMessage {
                message: proposal.message.to_bytes(),
                signature: *proposal.signature.inner(),
            }),
        }
    }
}

impl From<RawSignedConsensusMsg> for SignedConsensusMsg<TestContext> {
    fn from(value: RawSignedConsensusMsg) -> Self {
        match value {
            RawSignedConsensusMsg::Vote(vote) => SignedConsensusMsg::Vote(SignedVote {
                message: Vote::from_bytes(&vote.message).unwrap(),
                signature: vote.signature.into(),
            }),
            RawSignedConsensusMsg::Proposal(proposal) => {
                SignedConsensusMsg::Proposal(SignedProposal {
                    message: Proposal::from_bytes(&proposal.message).unwrap(),
                    signature: proposal.signature.into(),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawStreamMessage {
    pub stream_id: u64,
    pub sequence: u64,
    pub content: RawStreamContent,
}

#[derive(Serialize, Deserialize)]
pub enum RawStreamContent {
    Data(ProposalPart),
    Fin(bool),
}

impl From<StreamMessage<ProposalPart>> for RawStreamMessage {
    fn from(value: StreamMessage<ProposalPart>) -> Self {
        Self {
            stream_id: value.stream_id,
            sequence: value.sequence,
            content: match value.content {
                StreamContent::Data(proposal_part) => RawStreamContent::Data(proposal_part),
                StreamContent::Fin(fin) => RawStreamContent::Fin(fin),
            },
        }
    }
}

impl From<RawStreamMessage> for StreamMessage<ProposalPart> {
    fn from(value: RawStreamMessage) -> Self {
        Self {
            stream_id: value.stream_id,
            sequence: value.sequence,
            content: match value.content {
                RawStreamContent::Data(proposal_part) => StreamContent::Data(proposal_part),
                RawStreamContent::Fin(fin) => StreamContent::Fin(fin),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawStatus {
    pub peer_id: Vec<u8>,
    pub height: Height,
    pub history_min_height: Height,
}

impl From<Status<TestContext>> for RawStatus {
    fn from(value: Status<TestContext>) -> Self {
        Self {
            peer_id: value.peer_id.to_bytes(),
            height: value.height,
            history_min_height: value.history_min_height,
        }
    }
}

impl From<RawStatus> for Status<TestContext> {
    fn from(value: RawStatus) -> Self {
        Self {
            peer_id: PeerId::from_bytes(&value.peer_id).unwrap(),
            height: value.height,
            history_min_height: value.history_min_height,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ValueRawRequest {
    pub height: Height,
}

#[derive(Serialize, Deserialize)]
pub struct VoteSetRawRequest {
    pub height: Height,
    #[serde(with = "RoundDef")]
    pub round: Round,
}

#[derive(Serialize, Deserialize)]
pub enum RawRequest {
    SyncRequest(ValueRawRequest),
    VoteSetRequest(VoteSetRawRequest),
}

impl From<Request<TestContext>> for RawRequest {
    fn from(value: Request<TestContext>) -> Self {
        match value {
            Request::ValueRequest(block_request) => Self::SyncRequest(ValueRawRequest {
                height: block_request.height,
            }),
            Request::VoteSetRequest(vote_set_request) => Self::VoteSetRequest(VoteSetRawRequest {
                height: vote_set_request.height,
                round: vote_set_request.round,
            }),
        }
    }
}

impl From<RawRequest> for Request<TestContext> {
    fn from(value: RawRequest) -> Self {
        match value {
            RawRequest::SyncRequest(block_raw_request) => Self::ValueRequest(ValueRequest {
                height: block_raw_request.height,
            }),
            RawRequest::VoteSetRequest(vote_set_raw_request) => {
                Self::VoteSetRequest(VoteSetRequest {
                    height: vote_set_raw_request.height,
                    round: vote_set_raw_request.round,
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawExtension {
    pub data: Bytes,
}

#[derive(Serialize, Deserialize)]
pub struct RawSignedExtension {
    pub extension: RawExtension,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize)]
pub struct RawCommitSignature {
    pub address: Address,
    pub signature: Signature,
    pub extension: Option<RawSignedExtension>,
}

#[derive(Serialize, Deserialize)]
pub struct RawAggregatedSignature {
    pub signatures: Vec<RawCommitSignature>,
}

#[derive(Serialize, Deserialize)]
pub struct RawCommitCertificate {
    pub height: Height,
    #[serde(with = "RoundDef")]
    pub round: Round,
    pub value_id: ValueId,
    pub aggregated_signature: RawAggregatedSignature,
}

#[derive(Serialize, Deserialize)]
pub struct RawSyncedValue {
    pub value_bytes: Bytes,
    pub certificate: RawCommitCertificate,
}

#[derive(Serialize, Deserialize)]
pub struct ValueRawResponse {
    pub height: Height,
    pub block: Option<RawSyncedValue>,
}

impl From<ValueResponse<TestContext>> for ValueRawResponse {
    fn from(value: ValueResponse<TestContext>) -> Self {
        Self {
            height: value.height,
            block: value.value.map(|block| RawSyncedValue {
                value_bytes: block.value_bytes,
                certificate: RawCommitCertificate {
                    height: block.certificate.height,
                    round: block.certificate.round,
                    value_id: block.certificate.value_id,
                    aggregated_signature: RawAggregatedSignature {
                        signatures: block
                            .certificate
                            .aggregated_signature
                            .signatures
                            .iter()
                            .map(|sig| RawCommitSignature {
                                address: sig.address,
                                signature: *sig.signature.inner(),
                                extension: sig.extension.as_ref().map(|ext| RawSignedExtension {
                                    extension: RawExtension {
                                        data: ext.message.data.clone(),
                                    },
                                    signature: *ext.signature.inner(),
                                }),
                            })
                            .collect(),
                    },
                },
            }),
        }
    }
}

impl From<ValueRawResponse> for ValueResponse<TestContext> {
    fn from(value: ValueRawResponse) -> Self {
        Self {
            height: value.height,
            value: value.block.map(|block| DecidedValue {
                value_bytes: block.value_bytes,
                certificate: CommitCertificate {
                    height: block.certificate.height,
                    round: block.certificate.round,
                    value_id: block.certificate.value_id,
                    aggregated_signature: AggregatedSignature {
                        signatures: block
                            .certificate
                            .aggregated_signature
                            .signatures
                            .iter()
                            .map(|sig| CommitSignature {
                                address: sig.address,
                                signature: sig.signature.into(),
                                extension: sig.extension.as_ref().map(|ext| SignedExtension {
                                    message: Extension {
                                        data: ext.extension.data.clone(),
                                    },
                                    signature: ext.signature.into(),
                                }),
                            })
                            .collect(),
                    },
                },
            }),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawVoteSet {
    vote_set: Vec<RawSignedMessage>,
}

impl From<VoteSet<TestContext>> for RawVoteSet {
    fn from(value: VoteSet<TestContext>) -> Self {
        Self {
            vote_set: value
                .votes
                .iter()
                .map(|vote| RawSignedMessage {
                    message: vote.message.to_bytes(),
                    signature: *vote.signature.inner(),
                })
                .collect(),
        }
    }
}

impl From<RawVoteSet> for VoteSet<TestContext> {
    fn from(value: RawVoteSet) -> Self {
        Self {
            votes: value
                .vote_set
                .iter()
                .map(|vote| SignedVote {
                    message: Vote::from_bytes(&vote.message).unwrap(),
                    signature: vote.signature.into(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct VoteSetRawResponse {
    pub height: Height,
    #[serde(with = "RoundDef")]
    pub round: Round,
    pub vote_set: RawVoteSet,
}

impl From<VoteSetResponse<TestContext>> for VoteSetRawResponse {
    fn from(value: VoteSetResponse<TestContext>) -> Self {
        Self {
            height: value.height,
            round: value.round,
            vote_set: value.vote_set.into(),
        }
    }
}

impl From<VoteSetRawResponse> for VoteSetResponse<TestContext> {
    fn from(value: VoteSetRawResponse) -> Self {
        Self {
            height: value.height,
            round: value.round,
            vote_set: value.vote_set.into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum RawResponse {
    ValueResponse(ValueRawResponse),
    VoteSetResponse(VoteSetRawResponse),
}

impl From<Response<TestContext>> for RawResponse {
    fn from(value: Response<TestContext>) -> Self {
        match value {
            Response::ValueResponse(block_response) => Self::ValueResponse(block_response.into()),
            Response::VoteSetResponse(vote_set_response) => {
                Self::VoteSetResponse(vote_set_response.into())
            }
        }
    }
}

impl From<RawResponse> for Response<TestContext> {
    fn from(value: RawResponse) -> Self {
        match value {
            RawResponse::ValueResponse(block_raw_response) => {
                Self::ValueResponse(block_raw_response.into())
            }
            RawResponse::VoteSetResponse(vote_set_raw_response) => {
                Self::VoteSetResponse(vote_set_raw_response.into())
            }
        }
    }
}
