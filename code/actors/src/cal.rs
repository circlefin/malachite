use std::time::Duration;

use malachite_common::{Context, Height, Round, Validator, ValidatorSet};
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

    GetProposer {
        height: Ctx::Height,
        round: Round,
        reply: RpcReplyPort<Ctx::Address>,
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

    async fn get_proposer(
        &self,
        height: Ctx::Height,
        round: Round,
    ) -> Result<Ctx::Address, ActorProcessingErr> {
        assert!(self.validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let height = height.as_u64() as usize;
        let round = round.as_i64() as usize;

        let proposer_index = (height - 1 + round) % self.validator_set.count();
        let proposer = self.validator_set.get_by_index(proposer_index).unwrap();

        Ok(proposer.address().clone())
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

            Msg::GetProposer {
                height,
                round,
                reply,
            } => {
                let proposer = self.get_proposer(height, round).await?;
                reply.send(proposer)?;
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
