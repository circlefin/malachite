use std::marker::PhantomData;
use std::time::Duration;

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr};
use tracing::info;

use malachite_actors::consensus::ConsensusRef;
use malachite_actors::host::{HostMsg, LocallyProposedValue, ReceivedProposedValue};
use malachite_actors::prelude::ActorRef;
use malachite_common::{Context, Round};

use crate::value_builder::ValueBuilder;

pub struct State<Ctx: Context> {
    validator_set: Ctx::ValidatorSet,
    value_builder: Box<dyn ValueBuilder<Ctx>>,
}

pub struct Args<Ctx: Context> {
    validator_set: Ctx::ValidatorSet,
    value_builder: Box<dyn ValueBuilder<Ctx>>,
}

pub struct Host<Ctx: Context> {
    marker: PhantomData<Ctx>,
}

impl<Ctx> Host<Ctx>
where
    Ctx: Context,
{
    pub async fn spawn(
        value_builder: Box<dyn ValueBuilder<Ctx>>,
        validator_set: Ctx::ValidatorSet,
    ) -> Result<ActorRef<HostMsg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self {
                marker: PhantomData,
            },
            Args {
                validator_set,
                value_builder,
            },
        )
        .await?;

        Ok(actor_ref)
    }

    async fn get_value(
        &self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        consensus: ConsensusRef<Ctx>,
        value_builder: &mut dyn ValueBuilder<Ctx>,
    ) -> Result<LocallyProposedValue<Ctx>, ActorProcessingErr> {
        let value = value_builder
            .build_value_locally(height, round, timeout_duration, address, consensus)
            .await;

        match value {
            Some(value) => Ok(value),
            None => Err(eyre!("Value Builder failed to produce a value").into()),
        }
    }

    async fn build_value(
        &self,
        block_part: Ctx::BlockPart,
        value_builder: &mut dyn ValueBuilder<Ctx>,
    ) -> Result<Option<ReceivedProposedValue<Ctx>>, ActorProcessingErr> {
        let value = value_builder.build_value_from_block_parts(block_part).await;

        if let Some(value) = &value {
            info!("Value Builder received all parts, produced value for proposal: {value:?}",);
        }

        Ok(value)
    }
}

#[async_trait]
impl<Ctx: Context> Actor for Host<Ctx> {
    type Msg = HostMsg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args<Ctx>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(State {
            validator_set: args.validator_set,
            value_builder: args.value_builder,
        })
    }

    #[tracing::instrument(name = "host", skip(self, _myself, msg, state))]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                consensus,
                address,
                reply_to,
            } => {
                let value = self
                    .get_value(
                        height,
                        round,
                        timeout_duration,
                        address,
                        consensus,
                        state.value_builder.as_mut(),
                    )
                    .await?;

                reply_to.send(value)?;
            }

            HostMsg::ReceivedBlockPart {
                block_part,
                reply_to,
            } => {
                if let Some(value) = self
                    .build_value(block_part, state.value_builder.as_mut())
                    .await?
                {
                    // Send the proposed value (from blockparts) to consensus/ Driver
                    reply_to.send(value)?;
                }
            }

            HostMsg::GetReceivedValue {
                height,
                round,
                reply_to,
            } => {
                let value = state
                    .value_builder
                    .maybe_received_value(height, round)
                    .await;

                reply_to.send(value)?;
            }

            HostMsg::DecidedOnValue {
                height,
                round,
                value,
                commits,
            } => {
                let _v = state
                    .value_builder
                    .decided_on_value(height, round, value, commits)
                    .await;
            }

            HostMsg::GetValidatorSet {
                height: _,
                reply_to,
            } => {
                // FIXME: This is just a stub
                let validator_set = state.validator_set.clone();
                reply_to.send(validator_set)?;
            }
        }

        Ok(())
    }
}
