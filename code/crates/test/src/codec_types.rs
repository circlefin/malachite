use crate::{Address, Height, Proposal, ProposalPart, RoundDef, TestContext, ValueId, Vote};
use bytes::Bytes;
use malachite_actors::util::streaming::{StreamContent, StreamMessage};
use malachite_blocksync::{PeerId, Request, Response, Status, SyncedBlock};
use malachite_common::{
    AggregatedSignature, CommitCertificate, CommitSignature, Extension, Round, SignedExtension,
    SignedProposal, SignedVote,
};
use malachite_consensus::SignedConsensusMsg;
use malachite_proto::Protobuf;
use malachite_signing_ed25519::Signature;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct RawSignature {
    pub high: [u8; 32],
    pub low: [u8; 32],
}

impl RawSignature {
    pub fn to_vec(&self) -> Vec<u8> {
        [self.high, self.low].concat()
    }
    pub fn to_bytes(&self) -> Bytes {
        self.to_vec().into()
    }
}

impl From<Signature> for RawSignature {
    fn from(value: Signature) -> Self {
        Self {
            high: value.to_bytes().to_vec().try_into().unwrap(),
            low: value.to_bytes().to_vec().try_into().unwrap(),
        }
    }
}

impl From<RawSignature> for Signature {
    fn from(value: RawSignature) -> Self {
        Signature::from_bytes(value.to_vec()[..64].try_into().unwrap())
    }
}

#[derive(Serialize, Deserialize)]
pub struct RawSignedMessage {
    message: Bytes,
    signature: RawSignature,
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
                signature: vote.signature.into(),
            }),
            SignedConsensusMsg::Proposal(proposal) => Self::Proposal(RawSignedMessage {
                message: proposal.message.to_bytes(),
                signature: proposal.signature.into(),
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
    pub peer_id: PeerId,
    pub height: Height,
    pub earliest_block_height: Height,
}

impl From<Status<TestContext>> for RawStatus {
    fn from(value: Status<TestContext>) -> Self {
        Self {
            peer_id: value.peer_id,
            height: value.height,
            earliest_block_height: value.earliest_block_height,
        }
    }
}

impl From<RawStatus> for Status<TestContext> {
    fn from(value: RawStatus) -> Self {
        Self {
            peer_id: value.peer_id,
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
    pub signature: RawSignature,
}

#[derive(Serialize, Deserialize)]
pub struct RawCommitSignature {
    pub address: Address,
    pub signature: RawSignature,
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
                                signature: sig.signature.into(),
                                extension: sig.extension.as_ref().map(|ext| RawSignedExtension {
                                    extension: RawExtension {
                                        data: ext.message.data.clone(),
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
                                signature: sig.signature.clone().into(),
                                extension: sig.extension.as_ref().map(|ext| SignedExtension {
                                    message: Extension {
                                        data: ext.extension.data.clone(),
                                    },
                                    signature: ext.signature.clone().into(),
                                }),
                            })
                            .collect(),
                    },
                },
            }),
        }
    }
}
