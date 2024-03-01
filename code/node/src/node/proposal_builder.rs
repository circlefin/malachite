use std::marker::PhantomData;
use std::sync::atomic::AtomicPtr;
use std::time::Instant;

use malachite_common::{Context, Round};
use ractor::{Actor, RpcReplyPort};

use crate::value::ValueBuilder;

pub struct BuildProposal<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub deadline: Instant,
    pub reply: RpcReplyPort<ProposedValue<Ctx>>,
}

pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>,
}

pub struct ProposalBuilder<Ctx, VB> {
    builder: VB,
    marker: PhantomData<AtomicPtr<Ctx>>,
}

#[ractor::async_trait]
impl<Ctx, VB> Actor for ProposalBuilder<Ctx, VB>
where
    Ctx: Context,
    VB: ValueBuilder<Ctx>,
{
    type Msg = BuildProposal<Ctx>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ractor::ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ractor::ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ractor::ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ractor::ActorProcessingErr> {
        let BuildProposal {
            height,
            round,
            deadline,
            reply,
        } = msg;

        let value = self.builder.build_proposal(height, deadline).await;

        reply.send(ProposedValue {
            height,
            round,
            value,
        })?;

        Ok(())
    }
}
