use crate::{Address, Height, Proposal, ProposalPart, RoundDef, TestContext, ValueId, Vote};
use bytes::Bytes;
use ed25519_consensus::Signature;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_blocksync::{PeerId, Request, Response, Status, SyncedBlock};
use malachite_common::{
    AggregatedSignature, CommitCertificate, CommitSignature, Extension, Round, SignedExtension,
    SignedProposal, SignedVote,
};
use malachite_consensus::SignedConsensusMsg;
use malachite_proto::Protobuf;
use serde::{Deserialize, Serialize};

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
    pub earliest_block_height: Height,
}

impl From<Status<TestContext>> for RawStatus {
    fn from(value: Status<TestContext>) -> Self {
        Self {
            peer_id: value.peer_id.to_bytes(),
            height: value.height,
            earliest_block_height: value.earliest_block_height,
        }
    }
}

impl From<RawStatus> for Status<TestContext> {
    fn from(value: RawStatus) -> Self {
        Self {
            peer_id: PeerId::from_bytes(&value.peer_id).unwrap(),
            height: value.height,
            earliest_block_height: value.earliest_block_height,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawRequest {
    pub height: Height,
}

impl From<Request<TestContext>> for RawRequest {
    fn from(value: Request<TestContext>) -> Self {
        Self {
            height: value.height,
        }
    }
}

impl From<RawRequest> for Request<TestContext> {
    fn from(value: RawRequest) -> Self {
        Self {
            height: value.height,
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
pub struct RawSyncedBlock {
    pub block_bytes: Bytes,
    pub certificate: RawCommitCertificate,
}

#[derive(Serialize, Deserialize)]
pub struct RawResponse {
    pub height: Height,
    pub block: Option<RawSyncedBlock>,
}

impl From<Response<TestContext>> for RawResponse {
    fn from(value: Response<TestContext>) -> Self {
        Self {
            height: value.height,
            block: value.block.map(|block| RawSyncedBlock {
                block_bytes: block.block_bytes,
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

impl From<RawResponse> for Response<TestContext> {
    fn from(value: RawResponse) -> Self {
        Self {
            height: value.height,
            block: value.block.map(|block| SyncedBlock {
                block_bytes: block.block_bytes,
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
