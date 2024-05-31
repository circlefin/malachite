use std::marker::PhantomData;
use std::time::Duration;

use derive_where::derive_where;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use tracing::info;

use malachite_common::{Context, Round};
use malachite_driver::Validity;

use crate::consensus::Msg as ConsensusMsg;
use crate::util::PartStore;
use crate::util::ValueBuilder;

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

pub enum Msg<Ctx: Context> {
    // Request to build a local block/ value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        consensus: ActorRef<ConsensusMsg<Ctx>>,
        address: Ctx::Address,
        reply: RpcReplyPort<LocallyProposedValue<Ctx>>,
    },

    // BlockPart received <-- consensus <-- gossip
    BlockPart {
        block_part: Ctx::BlockPart,
        reply_to: ActorRef<ConsensusMsg<Ctx>>,
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
}

pub struct State<Ctx: Context> {
    part_store: PartStore<Ctx>,
    validator_set: Ctx::ValidatorSet,
}

pub struct Args<Ctx: Context> {
    part_store: PartStore<Ctx>,
    validator_set: Ctx::ValidatorSet,
}

pub struct Host<Ctx: Context> {
    value_builder: Box<dyn ValueBuilder<Ctx>>,
    marker: PhantomData<Ctx>,
}

impl<Ctx> Host<Ctx>
where
    Ctx: Context,
{
    pub async fn spawn(
        value_builder: Box<dyn ValueBuilder<Ctx>>,
        part_store: PartStore<Ctx>,
        validator_set: Ctx::ValidatorSet,
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self {
                value_builder,
                marker: PhantomData,
            },
            Args {
                part_store,
                validator_set,
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
        consensus: ActorRef<ConsensusMsg<Ctx>>,
        part_store: &mut PartStore<Ctx>,
    ) -> Result<LocallyProposedValue<Ctx>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value_locally(
                height,
                round,
                timeout_duration,
                address,
                consensus,
                part_store,
            )
            .await;

        match value {
            Some(value) => Ok(value),
            None => {
                todo!()
            }
        }
    }

    async fn build_value(
        &self,
        block_part: Ctx::BlockPart,
        part_store: &mut PartStore<Ctx>,
    ) -> Result<Option<ReceivedProposedValue<Ctx>>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value_from_block_parts(block_part, part_store)
            .await;

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
            part_store: args.part_store,
            validator_set: args.validator_set,
        })
    }

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
                        &mut state.part_store,
                    )
                    .await?;

                reply.send(value)?;
            }

            Msg::BlockPart {
                block_part,
                reply_to,
            } => {
                let maybe_block = self.build_value(block_part, &mut state.part_store).await?;

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
                let value = self
                    .value_builder
                    .maybe_received_value(height, round, &mut state.part_store)
                    .await;

                reply_to.send(value)?;
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
