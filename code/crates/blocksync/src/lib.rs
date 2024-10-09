use derive_where::derive_where;
use displaydoc::Display;
use libp2p::identity::PeerId;

use malachite_common::{Context, Round, SignedVote};

mod behaviour;
pub use behaviour::{Behaviour, Event};

mod codec;
pub use codec::NetworkCodec;

mod metrics;
pub use metrics::Metrics;

#[derive(Display)]
#[displaydoc("Status {{ peer_id: {peer_id}, height: {height}, round: {round} }}")]
#[derive_where(Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub height: Ctx::Height,
    pub round: Round,
}

#[derive(Debug)]
pub struct Request<Ctx: Context> {
    pub height: Ctx::Height,
}

#[derive(Debug)]
pub struct Response<Ctx: Context> {
    pub height: Ctx::Height,
    pub commits: Vec<SignedVote<Ctx>>,
    pub block_bytes: Vec<u8>,
}
