use bytes::Bytes;
use ed25519_consensus::Signature;
use serde::{Deserialize, Serialize};

use malachitebft_app::streaming::StreamId;
use malachitebft_core_consensus::{LivenessMsg, SignedConsensusMsg};
use malachitebft_core_types::{
    CommitCertificate, CommitSignature, NilOrVal, PolkaCertificate, PolkaSignature, Round,
    RoundCertificate, RoundCertificateType, RoundSignature, SignedProposal, SignedVote, VoteType,
};
use malachitebft_engine::util::streaming::{StreamContent, StreamMessage};
use malachitebft_proto::Protobuf;
use malachitebft_sync::{
    PeerId, RawDecidedValue, Request, Response, Status, ValueRequest, ValueResponse,
};

use crate::{Address, Height, Proposal, ProposalPart, TestContext, ValueId, Vote};

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
                message: vote.message.to_sign_bytes(),
                signature: *vote.signature.inner(),
            }),
            SignedConsensusMsg::Proposal(proposal) => Self::Proposal(RawSignedMessage {
                message: proposal.message.to_sign_bytes(),
                signature: *proposal.signature.inner(),
            }),
        }
    }
}

impl From<RawSignedConsensusMsg> for SignedConsensusMsg<TestContext> {
    fn from(value: RawSignedConsensusMsg) -> Self {
        match value {
            RawSignedConsensusMsg::Vote(vote) => SignedConsensusMsg::Vote(SignedVote {
                message: Vote::from_sign_bytes(&vote.message).unwrap(),
                signature: vote.signature.into(),
            }),
            RawSignedConsensusMsg::Proposal(proposal) => {
                SignedConsensusMsg::Proposal(SignedProposal {
                    message: Proposal::from_sign_bytes(&proposal.message).unwrap(),
                    signature: proposal.signature.into(),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "StreamId")]
pub struct RawStreamId(#[serde(getter = "StreamId::to_bytes")] Bytes);

impl From<RawStreamId> for StreamId {
    fn from(value: RawStreamId) -> Self {
        Self::new(value.0)
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawStreamMessage {
    #[serde(with = "RawStreamId")]
    pub stream_id: StreamId,
    pub sequence: u64,
    pub content: RawStreamContent,
}

#[derive(Serialize, Deserialize)]
pub enum RawStreamContent {
    Data(ProposalPart),
    Fin,
}

impl From<StreamMessage<ProposalPart>> for RawStreamMessage {
    fn from(value: StreamMessage<ProposalPart>) -> Self {
        Self {
            stream_id: value.stream_id,
            sequence: value.sequence,
            content: match value.content {
                StreamContent::Data(proposal_part) => RawStreamContent::Data(proposal_part),
                StreamContent::Fin => RawStreamContent::Fin,
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
                RawStreamContent::Fin => StreamContent::Fin,
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawStatus {
    pub peer_id: PeerId,
    pub tip_height: Height,
    pub history_min_height: Height,
}

impl From<Status<TestContext>> for RawStatus {
    fn from(value: Status<TestContext>) -> Self {
        Self {
            peer_id: value.peer_id,
            tip_height: value.tip_height,
            history_min_height: value.history_min_height,
        }
    }
}

impl From<RawStatus> for Status<TestContext> {
    fn from(value: RawStatus) -> Self {
        Self {
            peer_id: value.peer_id,
            tip_height: value.tip_height,
            history_min_height: value.history_min_height,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct ValueRawRequest {
    pub height: Height,
    pub end_height: Option<Height>,
}

#[derive(Serialize, Deserialize)]
pub enum RawRequest {
    SyncRequest(ValueRawRequest),
}

impl From<Request<TestContext>> for RawRequest {
    fn from(value: Request<TestContext>) -> Self {
        match value {
            Request::ValueRequest(request) => Self::SyncRequest(ValueRawRequest {
                height: *request.range.start(),
                end_height: Some(*request.range.end()),
            }),
        }
    }
}

impl From<RawRequest> for Request<TestContext> {
    fn from(value: RawRequest) -> Self {
        match value {
            RawRequest::SyncRequest(raw_request) => Self::ValueRequest(ValueRequest {
                range: raw_request.height..=raw_request.end_height.unwrap_or(raw_request.height),
            }),
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
}

#[derive(Serialize, Deserialize)]
pub struct RawCommitSignatures {
    pub signatures: Vec<RawCommitSignature>,
}

#[derive(Serialize, Deserialize)]
pub struct RawCommitCertificate {
    pub height: Height,
    pub round: Round,
    pub value_id: ValueId,
    pub commit_signatures: RawCommitSignatures,
}

impl From<RawCommitCertificate> for CommitCertificate<TestContext> {
    fn from(value: RawCommitCertificate) -> Self {
        CommitCertificate {
            height: value.height,
            round: value.round,
            value_id: value.value_id,
            commit_signatures: value
                .commit_signatures
                .signatures
                .iter()
                .map(|sig| CommitSignature {
                    address: sig.address,
                    signature: sig.signature.into(),
                })
                .collect(),
        }
    }
}

impl From<CommitCertificate<TestContext>> for RawCommitCertificate {
    fn from(value: CommitCertificate<TestContext>) -> Self {
        Self {
            height: value.height,
            round: value.round,
            value_id: value.value_id,
            commit_signatures: RawCommitSignatures {
                signatures: value
                    .commit_signatures
                    .iter()
                    .map(|sig| RawCommitSignature {
                        address: sig.address,
                        signature: *sig.signature.inner(),
                    })
                    .collect(),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawSyncedValue {
    pub value_bytes: Bytes,
    pub certificate: RawCommitCertificate,
}

#[derive(Serialize, Deserialize)]
pub struct ValueRawResponse {
    pub start_height: Height,
    pub value: Vec<RawSyncedValue>,
}

impl From<ValueResponse<TestContext>> for ValueRawResponse {
    fn from(response: ValueResponse<TestContext>) -> Self {
        Self {
            start_height: response.start_height,
            value: response
                .values
                .into_iter()
                .map(|value| RawSyncedValue {
                    value_bytes: value.value_bytes,
                    certificate: value.certificate.into(),
                })
                .collect(),
        }
    }
}

impl From<ValueRawResponse> for ValueResponse<TestContext> {
    fn from(response: ValueRawResponse) -> Self {
        Self {
            start_height: response.start_height,
            values: response
                .value
                .into_iter()
                .map(|value| RawDecidedValue {
                    value_bytes: value.value_bytes,
                    certificate: value.certificate.into(),
                })
                .collect(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum RawResponse {
    ValueResponse(ValueRawResponse),
}

impl From<Response<TestContext>> for RawResponse {
    fn from(value: Response<TestContext>) -> Self {
        match value {
            Response::ValueResponse(block_response) => Self::ValueResponse(block_response.into()),
        }
    }
}

impl From<RawResponse> for Response<TestContext> {
    fn from(value: RawResponse) -> Self {
        match value {
            RawResponse::ValueResponse(block_raw_response) => {
                Self::ValueResponse(block_raw_response.into())
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawPolkaSignature {
    pub address: Address,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize)]
pub struct RawPolkaCertificate {
    pub height: Height,
    pub round: Round,
    pub value_id: ValueId,
    pub polka_signatures: Vec<RawPolkaSignature>,
}

#[derive(Serialize, Deserialize)]
pub struct RawRoundSignature {
    pub vote_type: VoteType,
    pub value_id: NilOrVal<ValueId>,
    pub address: Address,
    pub signature: Signature,
}

#[derive(Serialize, Deserialize)]
pub struct RawRoundCertificate {
    pub height: Height,
    pub round: Round,
    pub cert_type: RoundCertificateType,
    pub round_signatures: Vec<RawRoundSignature>,
}

#[derive(Serialize, Deserialize)]
pub enum RawLivenessMsg {
    Vote(RawSignedMessage),
    PolkaCertificate(RawPolkaCertificate),
    SkipRoundCertificate(RawRoundCertificate),
}

impl From<LivenessMsg<TestContext>> for RawLivenessMsg {
    fn from(value: LivenessMsg<TestContext>) -> Self {
        match value {
            LivenessMsg::Vote(vote) => Self::Vote(RawSignedMessage {
                message: vote.message.to_sign_bytes(),
                signature: *vote.signature.inner(),
            }),
            LivenessMsg::PolkaCertificate(polka) => Self::PolkaCertificate(RawPolkaCertificate {
                height: polka.height,
                round: polka.round,
                value_id: polka.value_id,
                polka_signatures: vec![], // Placeholder, implement as needed
            }),
            LivenessMsg::SkipRoundCertificate(round_cert) => {
                Self::SkipRoundCertificate(RawRoundCertificate {
                    height: round_cert.height,
                    round: round_cert.round,
                    cert_type: round_cert.cert_type,
                    round_signatures: round_cert
                        .round_signatures
                        .into_iter()
                        .map(|sig| RawRoundSignature {
                            vote_type: sig.vote_type,
                            value_id: sig.value_id,
                            address: sig.address,
                            signature: *sig.signature.inner(),
                        })
                        .collect(),
                })
            }
        }
    }
}

impl From<RawLivenessMsg> for LivenessMsg<TestContext> {
    fn from(value: RawLivenessMsg) -> Self {
        match value {
            RawLivenessMsg::Vote(vote) => LivenessMsg::Vote(SignedVote {
                message: Vote::from_bytes(&vote.message).unwrap(),
                signature: vote.signature.into(),
            }),
            RawLivenessMsg::PolkaCertificate(cert) => {
                LivenessMsg::PolkaCertificate(PolkaCertificate {
                    height: cert.height,
                    round: cert.round,
                    value_id: cert.value_id,
                    polka_signatures: cert
                        .polka_signatures
                        .into_iter()
                        .map(|sig| PolkaSignature {
                            address: sig.address,
                            signature: sig.signature.into(),
                        })
                        .collect(),
                })
            }
            RawLivenessMsg::SkipRoundCertificate(cert) => {
                LivenessMsg::SkipRoundCertificate(RoundCertificate {
                    height: cert.height,
                    round: cert.round,
                    cert_type: cert.cert_type,
                    round_signatures: cert
                        .round_signatures
                        .into_iter()
                        .map(|sig| RoundSignature {
                            vote_type: sig.vote_type,
                            value_id: sig.value_id,
                            address: sig.address,
                            signature: sig.signature.into(),
                        })
                        .collect(),
                })
            }
        }
    }
}
