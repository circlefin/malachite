use std::time::Duration;

use malachite_common::{Context, Round};
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::util::ValueBuilder;

pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>,
}

pub enum Msg<Ctx: Context> {
    // Initialize the builder state with the gossip actor
    Init {
        gossip_actor: ActorRef<crate::consensus::Msg<Ctx>>,
    },

    // Request for a value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        reply: RpcReplyPort<ProposedValue<Ctx>>,
        address: Ctx::Address,
    },

    // BlockPart received <-- consensus <-- gossip
    BlockPart(Ctx::BlockPart),
}

pub struct State<Ctx>
where
    Ctx: Context,
{
    gossip_actor: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
}

pub struct ProposalBuilder<Ctx: Context> {
    #[allow(dead_code)]
    ctx: Ctx,
    value_builder: Box<dyn ValueBuilder<Ctx>>,
}

impl<Ctx: Context> ProposalBuilder<Ctx> {
    pub async fn spawn(
        ctx: Ctx,
        value_builder: Box<dyn ValueBuilder<Ctx>>,
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(None, Self { ctx, value_builder }, ()).await?;

        Ok(actor_ref)
    }

    async fn get_value(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        gossip_actor: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
    ) -> Result<ProposedValue<Ctx>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value_locally(height, round, timeout_duration, address, gossip_actor)
            .await;

        Ok(ProposedValue {
            height,
            round,
            value,
        })
    }

    async fn build_value(
        &self,
        block_part: Ctx::BlockPart,
    ) -> Result<Option<ProposedValue<Ctx>>, ActorProcessingErr> {
        // TODO
        let _ = self
            .value_builder
            .build_value_from_block_parts(block_part)
            .await;
        Ok(None)
    }
}

#[async_trait]
impl<Ctx: Context> Actor for ProposalBuilder<Ctx> {
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(State { gossip_actor: None })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Init { gossip_actor } => {
                state.gossip_actor = Some(gossip_actor);
            }

            Msg::GetValue {
                height,
                round,
                timeout_duration,
                reply,
                address,
            } => {
                let value = self
                    .get_value(
                        height,
                        round,
                        timeout_duration,
                        address,
                        state.gossip_actor.clone(),
                    )
                    .await?;
                reply.send(value)?;
            }

            Msg::BlockPart(block_part) => {
                self.build_value(block_part).await?;
            }
        }

        Ok(())
    }
}
