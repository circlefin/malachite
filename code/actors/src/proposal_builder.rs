use std::marker::PhantomData;
use std::sync::atomic::AtomicPtr;
use std::time::Instant;

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorRef, RpcReplyPort};

use malachite_common::{Context, Round};
use malachite_node::value_builder::ValueBuilder;

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

pub struct ProposalBuilder<Ctx> {
    builder: Box<dyn ValueBuilder<Ctx>>,
    marker: PhantomData<AtomicPtr<Ctx>>,
}

impl<Ctx> ProposalBuilder<Ctx>
where
    Ctx: Context,
{
    pub async fn spawn(
        builder: Box<dyn ValueBuilder<Ctx>>,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<BuildProposal<Ctx>>, ractor::SpawnErr> {
        let this = Self {
            builder,
            marker: PhantomData,
        };

        let (actor_ref, _) = if let Some(supervisor) = supervisor {
            Actor::spawn_linked(None, this, (), supervisor).await?
        } else {
            Actor::spawn(None, this, ()).await?
        };

        Ok(actor_ref)
    }
}

#[async_trait]
impl<Ctx> Actor for ProposalBuilder<Ctx>
where
    Ctx: Context,
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
