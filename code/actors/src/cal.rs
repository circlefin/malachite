use malachite_common::Context;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

pub enum Msg<Ctx: Context> {
    GetValidatorSet {
        height: Ctx::Height,
        reply: RpcReplyPort<Ctx::ValidatorSet>,
    },
}

pub struct CAL<Ctx: Context> {
    #[allow(dead_code)]
    ctx: Ctx,
    validator_set: Ctx::ValidatorSet,
}

impl<Ctx: Context> CAL<Ctx> {
    pub async fn spawn(
        ctx: Ctx,
        validator_set: Ctx::ValidatorSet,
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(None, Self { ctx, validator_set }, ()).await?;

        Ok(actor_ref)
    }

    async fn get_validator_set(
        &self,
        _height: Ctx::Height,
    ) -> Result<Ctx::ValidatorSet, ActorProcessingErr> {
        Ok(self.validator_set.clone())
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
        }

        Ok(())
    }
}
