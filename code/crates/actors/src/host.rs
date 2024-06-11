use std::marker::PhantomData;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tracing::info;

use malachite_common::{Context, Round};
use malachite_driver::Validity;

use crate::consensus::{ConsensusRef, Msg as ConsensusMsg};
use crate::value_builder::ValueBuilder;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>, // todo - should we remove?
}

/// Input to the round state machine.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ReceivedProposedValue<Ctx: Context> {
    pub validator_address: Ctx::Address,
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>,
    pub valid: Validity,
}

pub type HostRef<Ctx> = ActorRef<Msg<Ctx>>;

pub enum Msg<Ctx: Context> {
    // Request to build a local block/ value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        consensus: ConsensusRef<Ctx>,
        address: Ctx::Address,
        reply: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    // BlockPart received <-- consensus <-- gossip
    BlockPart {
        block_part: Ctx::BlockPart,
        reply_to: ConsensusRef<Ctx>,
    },

    // Retrieve a block/ value for which all parts have been received
    GetReceivedValue {
        height: Ctx::Height,
        round: Round,
        reply_to: RpcReplyPort<Option<ReceivedProposedValue<Ctx>>>,
    },

    GetValidatorSet {
        height: Ctx::Height,
        reply_to: RpcReplyPort<Ctx::ValidatorSet>,
    },

    // Decided value
    DecidedOnValue {
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
    },
}

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
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
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
            None => Err(eyre::eyre!("Value Builder failed to produce a value").into()),
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
    type Msg = Msg<Ctx>;
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
            Msg::GetValue {
                height,
                round,
                timeout_duration,
                consensus,
                reply,
                address,
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

                reply.send(value)?;
            }

            Msg::BlockPart {
                block_part,
                reply_to,
            } => {
                let maybe_block = self
                    .build_value(block_part, state.value_builder.as_mut())
                    .await?;

                // Send the proposed value (from blockparts) to consensus/ Driver
                if let Some(value_assembled) = maybe_block {
                    reply_to.cast(ConsensusMsg::BlockReceived(value_assembled))?;
                }
            }

            Msg::GetReceivedValue {
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

            Msg::DecidedOnValue {
                height,
                round,
                value,
            } => {
                info!("what");
                let _v = state
                    .value_builder
                    .decided_on_value(height, round, value)
                    .await;
            }

            Msg::GetValidatorSet {
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
