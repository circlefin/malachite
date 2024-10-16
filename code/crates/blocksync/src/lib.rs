use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use libp2p::identity::PeerId;
use libp2p::request_response::{InboundRequestId, OutboundRequestId};
use serde::{Deserialize, Serialize};

use malachite_common::{Certificate, Context, SignedProposal};

mod behaviour;
pub use behaviour::{Behaviour, Event};

mod codec;
pub use codec::NetworkCodec;

mod metrics;
pub use metrics::Metrics;

mod state;
pub use state::State;

pub type ResponseChannel = libp2p::request_response::ResponseChannel<RawResponse>;

#[derive(Display)]
#[displaydoc("Status {{ peer_id: {peer_id}, height: {height} }}")]
#[derive_where(Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub height: Ctx::Height,
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
