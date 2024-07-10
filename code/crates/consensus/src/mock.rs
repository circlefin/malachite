use std::marker::PhantomData;

use derive_where::derive_where;

#[derive(Debug)]
pub struct PeerId;

#[derive(Debug)]
pub struct Multiaddr;

#[derive_where(Debug)]
pub enum NetworkMsg<Ctx> {
    Empty(PhantomData<Ctx>),
}

#[derive_where(Debug)]
pub struct GossipEvent<Ctx> {
    marker: PhantomData<Ctx>,
}

#[derive_where(Debug)]
pub struct ReceivedProposedValue<Ctx> {
    marker: PhantomData<Ctx>,
}
