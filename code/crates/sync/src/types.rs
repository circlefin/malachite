use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use libp2p::request_response;
use serde::{Deserialize, Serialize};

use malachite_common::{CommitCertificate, Context, Round, VoteSet};
pub use malachite_peer::PeerId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct InboundRequestId(String);

impl InboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct OutboundRequestId(String);

impl OutboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(id.to_string())
    }
}

pub type ResponseChannel = request_response::ResponseChannel<RawResponse>;

#[derive(Display)]
#[displaydoc("Status {{ peer_id: {peer_id}, height: {height} }}")]
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub height: Ctx::Height,
    pub history_min_height: Ctx::Height,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Request<Ctx: Context> {
    ValueRequest(ValueRequest<Ctx>),
    VoteSetRequest(VoteSetRequest<Ctx>),
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Response<Ctx: Context> {
    ValueResponse(ValueResponse<Ctx>),
    VoteSetResponse(VoteSetResponse<Ctx>),
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
    pub value: Option<DecidedValue<Ctx>>,
}

impl<Ctx: Context> ValueResponse<Ctx> {
    pub fn new(height: Ctx::Height, value: Option<DecidedValue<Ctx>>) -> Self {
        Self { height, value }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct DecidedValue<Ctx: Context> {
    pub value_bytes: Bytes,
    pub certificate: CommitCertificate<Ctx>,
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

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct VoteSetRequest<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
}

impl<Ctx: Context> VoteSetRequest<Ctx> {
    pub fn new(height: Ctx::Height, round: Round) -> Self {
        Self { height, round }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct VoteSetResponse<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub vote_set: VoteSet<Ctx>,
}

impl<Ctx: Context> VoteSetResponse<Ctx> {
    pub fn new(height: Ctx::Height, round: Round, vote_set: VoteSet<Ctx>) -> Self {
        Self {
            height,
            round,
            vote_set,
        }
    }
}
