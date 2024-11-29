use bytes::Bytes;
use malachite_actors::util::codec::NetworkCodec;

use crate::codec_types::{
    RawRequest, RawResponse, RawSignedConsensusMsg, RawStatus, RawStreamMessage,
};
use crate::{ProposalPart, TestContext};
use malachite_actors::util::streaming::StreamMessage;
use malachite_blocksync::{Request, Response, Status};
use malachite_consensus::SignedConsensusMsg;

pub struct TestCodec;

impl NetworkCodec<ProposalPart> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<ProposalPart, Self::Error> {
        serde_json::from_slice(&bytes)
    }

    fn encode(&self, msg: ProposalPart) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&msg).map(Bytes::from)
    }
}

impl NetworkCodec<SignedConsensusMsg<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<TestContext>, Self::Error> {
        serde_json::from_slice::<RawSignedConsensusMsg>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: SignedConsensusMsg<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec::<RawSignedConsensusMsg>(&msg.into()).map(Bytes::from)
    }
}

impl NetworkCodec<StreamMessage<ProposalPart>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<ProposalPart>, Self::Error> {
        serde_json::from_slice::<RawStreamMessage>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: StreamMessage<ProposalPart>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec::<RawStreamMessage>(&msg.into()).map(Bytes::from)
    }
}

impl NetworkCodec<Status<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Status<TestContext>, Self::Error> {
        serde_json::from_slice::<RawStatus>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: Status<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec::<RawStatus>(&msg.into()).map(Bytes::from)
    }
}

impl NetworkCodec<Request<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Request<TestContext>, Self::Error> {
        serde_json::from_slice::<RawRequest>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: Request<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec::<RawRequest>(&msg.into()).map(Bytes::from)
    }
}

impl NetworkCodec<Response<TestContext>> for TestCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Response<TestContext>, Self::Error> {
        serde_json::from_slice::<RawResponse>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: Response<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec::<RawResponse>(&msg.into()).map(Bytes::from)
    }
}
