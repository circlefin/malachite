pub mod raw;

use bytes::Bytes;
use malachitebft_codec::{Codec, HasEncodedLen};
use tracing::warn;

use malachitebft_core_consensus::{LivenessMsg, SignedConsensusMsg};
use malachitebft_engine::util::streaming::StreamMessage;
use malachitebft_sync::{Request, Response, Status};

use crate::{ProposalPart, TestContext, Value};

use raw::{
    RawLivenessMsg, RawRequest, RawResponse, RawSignedConsensusMsg, RawStatus, RawStreamMessage,
};

#[derive(Copy, Clone, Debug)]
pub struct JsonCodec;

impl Codec<Value> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Value, Self::Error> {
        serde_json::from_slice(&bytes)
    }

    fn encode(&self, msg: &Value) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&msg).map(Bytes::from)
    }
}

impl Codec<ProposalPart> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<ProposalPart, Self::Error> {
        serde_json::from_slice(&bytes)
    }

    fn encode(&self, msg: &ProposalPart) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&msg).map(Bytes::from)
    }
}

impl Codec<SignedConsensusMsg<TestContext>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<SignedConsensusMsg<TestContext>, Self::Error> {
        serde_json::from_slice::<RawSignedConsensusMsg>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &SignedConsensusMsg<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawSignedConsensusMsg::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<StreamMessage<ProposalPart>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<StreamMessage<ProposalPart>, Self::Error> {
        serde_json::from_slice::<RawStreamMessage>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &StreamMessage<ProposalPart>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawStreamMessage::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Status<TestContext>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Status<TestContext>, Self::Error> {
        serde_json::from_slice::<RawStatus>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Status<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawStatus::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Request<TestContext>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Request<TestContext>, Self::Error> {
        serde_json::from_slice::<RawRequest>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Request<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawRequest::from(msg.clone())).map(Bytes::from)
    }
}

impl Codec<Response<TestContext>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<Response<TestContext>, Self::Error> {
        serde_json::from_slice::<RawResponse>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &Response<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawResponse::from(msg.clone())).map(Bytes::from)
    }
}

impl HasEncodedLen<Response<TestContext>> for JsonCodec {
    fn encoded_len(
        &self,
        msg: &Response<TestContext>,
    ) -> Result<usize, <Self as Codec<Response<TestContext>>>::Error> {
        warn!("encoded_len serializes the data to compute the length; consider using ProtobufCodec if \
        you want to compute the length of the encoded data without encoding them");
        serde_json::to_vec(&RawResponse::from(msg.clone())).map(|b| b.len())
    }
}

impl Codec<LivenessMsg<TestContext>> for JsonCodec {
    type Error = serde_json::Error;

    fn decode(&self, bytes: Bytes) -> Result<LivenessMsg<TestContext>, Self::Error> {
        serde_json::from_slice::<RawLivenessMsg>(&bytes).map(Into::into)
    }

    fn encode(&self, msg: &LivenessMsg<TestContext>) -> Result<Bytes, Self::Error> {
        serde_json::to_vec(&RawLivenessMsg::from(msg.clone())).map(Bytes::from)
    }
}
