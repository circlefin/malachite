use bytes::Bytes;
use malachite_codec::Codec;

use crate::codec::types::{
    RawRequest, RawResponse, RawSignedConsensusMsg, RawStatus, RawStreamMessage,
};
use crate::{ProposalPart, TestContext};
use malachite_consensus::SignedConsensusMsg;
use malachite_engine::util::streaming::StreamMessage;
use malachite_sync::{Request, Response, Status};

pub struct TestCodec;

impl Codec<ProposalPart> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<ProposalPart, Self::Error> {
        serde_json::from_slice(&bytes)
    }

    fn encode(&self, msg: &ProposalPart) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&msg).map(Bytes::from)
    }
}

impl Codec<SignedConsensusMsg<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<TestContext>, Self::Error> {
        serde_json::from_slice::<RawSignedConsensusMsg>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &SignedConsensusMsg<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawSignedConsensusMsg::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<StreamMessage<ProposalPart>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<ProposalPart>, Self::Error> {
        serde_json::from_slice::<RawStreamMessage>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &StreamMessage<ProposalPart>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawStreamMessage::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Status<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Status<TestContext>, Self::Error> {
        serde_json::from_slice::<RawStatus>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Status<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawStatus::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Request<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Request<TestContext>, Self::Error> {
        serde_json::from_slice::<RawRequest>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Request<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawRequest::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Response<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Response<TestContext>, Self::Error> {
        serde_json::from_slice::<RawResponse>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Response<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawResponse::from(msg.clone())).map(Bytes::from)
    }
}
