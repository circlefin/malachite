use std::{ops::RangeInclusive, sync::Arc};

use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use libp2p::request_response;
use serde::{Deserialize, Serialize};

use malachitebft_core_types::{CommitCertificate, Context, Height};
pub use malachitebft_peer::PeerId;

/// Indicates whether the height is the start of a new height or a restart of the latest height
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HeightStartType {
    /// This is the start of a new height
    Start,

    /// This is a restart of the latest height
    Restart,
}

impl HeightStartType {
    pub const fn from_is_restart(is_restart: bool) -> Self {
        if is_restart {
            Self::Restart
        } else {
            Self::Start
        }
    }

    pub const fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }

    pub const fn is_restart(&self) -> bool {
        matches!(self, Self::Restart)
    }
}

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
    pub range: RangeInclusive<Ctx::Height>,
}

impl<Ctx: Context> ValueRequest<Ctx> {
    pub fn new(range: RangeInclusive<Ctx::Height>) -> Self {
        Self { range }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueResponse<Ctx: Context> {
    /// The height of the first value in the response.
    pub start_height: Ctx::Height,

    /// Values are sequentially ordered by height.
    pub values: Vec<RawDecidedValue<Ctx>>,
}

impl<Ctx: Context> ValueResponse<Ctx> {
    pub fn new(start_height: Ctx::Height, values: Vec<RawDecidedValue<Ctx>>) -> Self {
        Self {
            start_height,
            values,
        }
    }

    pub fn end_height(&self) -> Option<Ctx::Height> {
        if self.values.is_empty() {
            None
        } else {
            Some(self.start_height.increment_by(self.values.len() as u64 - 1))
        }
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
