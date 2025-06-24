use std::sync::Arc;

use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use libp2p::request_response;
use serde::{Deserialize, Serialize};

use malachitebft_core_types::{CommitCertificate, Context};
pub use malachitebft_peer::PeerId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct InboundRequestId(Arc<str>);

impl InboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(Arc::from(id.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct OutboundRequestId(Arc<str>);

impl OutboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(Arc::from(id.to_string()))
    }
}

pub type ResponseChannel = request_response::ResponseChannel<RawResponse>;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub tip_height: Ctx::Height,
    pub history_min_height: Ctx::Height,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Request<Ctx: Context> {
    ValueRequest(ValueRequest<Ctx>),
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Response<Ctx: Context> {
    ValueResponse(ValueResponse<Ctx>),
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueRequest<Ctx: Context> {
    pub height: Ctx::Height,
}

impl<Ctx: Context> ValueRequest<Ctx> {
    pub fn new(height: Ctx::Height) -> Self {
        Self { height }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueResponse<Ctx: Context> {
    pub height: Ctx::Height,
    pub value: Option<RawDecidedValue<Ctx>>,
}

impl<Ctx: Context> ValueResponse<Ctx> {
    pub fn new(height: Ctx::Height, value: Option<RawDecidedValue<Ctx>>) -> Self {
        Self { height, value }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct RawDecidedValue<Ctx: Context> {
    pub value_bytes: Bytes,
    pub certificate: CommitCertificate<Ctx>,
}

impl<Ctx: Context> RawDecidedValue<Ctx> {
    pub fn new(value_bytes: Bytes, certificate: CommitCertificate<Ctx>) -> Self {
        Self {
            value_bytes,
            certificate,
        }
    }
}

#[derive(Clone, Debug)]
pub enum RawMessage {
    Request {
        request_id: request_response::InboundRequestId,
        peer: PeerId,
        body: Bytes,
    },
    Response {
        request_id: request_response::OutboundRequestId,
        peer: PeerId,
        body: Bytes,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRequest(pub Bytes);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawResponse(pub Bytes);
