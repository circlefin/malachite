use std::time::Duration;
use tracing::info;

use malachite_common::{BlockPart, Context, Round};
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::util::ValueBuilder;

pub struct ProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>,
}

pub enum Msg<Ctx: Context> {
    // request from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        reply: RpcReplyPort<ProposedValue<Ctx>>,
        address: Ctx::Address,
    },
    BlockPart(Ctx::BlockPart),
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
        address: Ctx::Address, // TODO remove
    ) -> Result<ProposedValue<Ctx>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value(height, round, timeout_duration, address)
            .await;

        Ok(ProposedValue {
            height,
            round,
            value,
        })
    }

    async fn build_value(
        &self,
        _block_part: Ctx::BlockPart,
    ) -> Result<ProposedValue<Ctx>, ActorProcessingErr> {
        todo!()
    }
}

#[async_trait]
impl<Ctx: Context> Actor for ProposalBuilder<Ctx> {
    type Msg = Msg<Ctx>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        //Ok(self.value_builder.pre_start(myself).await?)
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::GetValue {
                height,
                round,
                timeout_duration,
                reply,
                address,
            } => {
                let value = self
                    .get_value(height, round, timeout_duration, address)
                    .await?;
                reply.send(value)?;
            }

            Msg::BlockPart(block_part) => {
                info!(
                    "Proposal Builder received a block part (h: {}, r:{}, seq: {})",
                    block_part.height(),
                    block_part.round(),
                    block_part.sequence()
                );

                self.build_value(block_part).await?;
            }
        }

        Ok(())
    }
}
