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
    GetValidatorSet {
        height: Ctx::Height,
        reply: RpcReplyPort<Ctx::ValidatorSet>,
    },

    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        reply: RpcReplyPort<ProposedValue<Ctx>>,
    },
}

pub struct CAL<Ctx: Context> {
    #[allow(dead_code)]
    ctx: Ctx,
    validator_set: Ctx::ValidatorSet,
    value_builder: Box<dyn ValueBuilder<Ctx>>,
}

impl<Ctx: Context> CAL<Ctx> {
    pub async fn spawn(
        ctx: Ctx,
        validator_set: Ctx::ValidatorSet,
        value_builder: Box<dyn ValueBuilder<Ctx>>,
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self {
                ctx,
                validator_set,
                value_builder,
            },
            (),
        )
        .await?;

        Ok(actor_ref)
    }

    async fn get_validator_set(
        &self,
        _height: Ctx::Height,
    ) -> Result<Ctx::ValidatorSet, ActorProcessingErr> {
        Ok(self.validator_set.clone())
    }

    async fn get_value(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
    ) -> Result<ProposedValue<Ctx>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value(height, timeout_duration)
            .await;

        Ok(ProposedValue {
            height,
            round,
            value,
        })
    }
}

#[async_trait]
impl<Ctx: Context> Actor for CAL<Ctx> {
    type Msg = Msg<Ctx>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::GetValidatorSet { height, reply } => {
                let validators = self.get_validator_set(height).await?;
                reply.send(validators)?;
            }

            Msg::GetValue {
                height,
                round,
                timeout_duration,
                reply,
            } => {
                let value = self.get_value(height, round, timeout_duration).await?;
                reply.send(value)?;
            }
        }

        Ok(())
    }
}
