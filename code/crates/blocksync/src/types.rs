use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use serde::{Deserialize, Serialize};

use malachite_common::{Certificate, Context, SignedProposal};

pub use libp2p::identity::PeerId;
pub use libp2p::request_response::{InboundRequestId, OutboundRequestId};

pub type ResponseChannel = libp2p::request_response::ResponseChannel<RawResponse>;

#[derive(Display)]
#[displaydoc("Status {{ peer_id: {peer_id}, height: {height} }}")]
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub height: Ctx::Height,
    pub earliest_block_height: Ctx::Height,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Request<Ctx: Context> {
    pub height: Ctx::Height,
}

impl<Ctx: Context> Request<Ctx> {
    pub fn new(height: Ctx::Height) -> Self {
        Self { height }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Response<Ctx: Context> {
    pub height: Ctx::Height,
    pub block: Option<SyncedBlock<Ctx>>,
}

impl<Ctx: Context> Response<Ctx> {
    pub fn new(height: Ctx::Height, block: Option<SyncedBlock<Ctx>>) -> Self {
        Self { height, block }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct SyncedBlock<Ctx: Context> {
    pub proposal: SignedProposal<Ctx>,
    pub certificate: Certificate<Ctx>,
    pub block_bytes: Bytes,
}

#[derive(Clone, Debug)]
pub enum RawMessage {
    Request {
        request_id: InboundRequestId,
        peer: PeerId,
        body: Bytes,
    },
    Response {
        request_id: OutboundRequestId,
        body: Bytes,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRequest(pub Bytes);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawResponse(pub Bytes);
