use std::fmt::Debug;
use std::time::Duration;
use tracing::info;

use derive_where::derive_where;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use malachite_common::{Context, Round};
use malachite_driver::Validity;

use crate::consensus::Msg as ConsensusMsg;
use crate::util::value_builder::test::PartStore;
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
    // Initialize the builder state with the gossip actor
    Init {
        consensus: ActorRef<ConsensusMsg<Ctx>>,
        part_store: PartStore,
    },

    // Request to build a local block/ value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        reply: RpcReplyPort<LocallyProposedValue<Ctx>>,
        address: Ctx::Address,
    },

    // BlockPart received <-- consensus <-- gossip
    BlockPart(Ctx::BlockPart),

    // Retrieve a block/ value for which all parts have been received
    GetReceivedValue {
        height: Ctx::Height,
        round: Round,
        reply: RpcReplyPort<Option<ReceivedProposedValue<Ctx>>>,
    },
}

pub struct State<Ctx: Context> {
    consensus: Option<ActorRef<ConsensusMsg<Ctx>>>,
    part_store: PartStore,
}

pub struct ProposalBuilder<Ctx: Context> {
    _ctx: Ctx,
    value_builder: Box<dyn ValueBuilder<Ctx>>,
}

impl<Ctx> ProposalBuilder<Ctx>
where
    Ctx: Context + Debug,
{
    pub async fn spawn(
        ctx: Ctx,
        value_builder: Box<dyn ValueBuilder<Ctx>>,
    ) -> Result<ActorRef<Msg<Ctx>>, ActorProcessingErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self {
                _ctx: ctx,
                value_builder,
            },
            (),
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
        gossip_actor: Option<ActorRef<ConsensusMsg<Ctx>>>,
        part_store: &mut PartStore,
    ) -> Result<LocallyProposedValue<Ctx>, ActorProcessingErr> {
        let value = self
            .value_builder
            .build_value_locally(
                height,
                round,
                timeout_duration,
                address,
                gossip_actor,
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
        part_store: &mut PartStore,
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
impl<Ctx: Context + std::fmt::Debug> Actor for ProposalBuilder<Ctx> {
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(State {
            consensus: None,
            part_store: PartStore::new(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Init {
                consensus,
                part_store,
            } => {
                state.consensus = Some(consensus);
                state.part_store = part_store
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
                        state.consensus.clone(),
                        &mut state.part_store,
                    )
                    .await?;
                reply.send(value)?;
            }

            Msg::BlockPart(block_part) => {
                let maybe_block = self.build_value(block_part, &mut state.part_store).await?;
                // Send the proposed value (from blockparts) to consensus/ Driver
                if let Some(value_assembled) = maybe_block {
                    state
                        .consensus
                        .as_ref()
                        .unwrap()
                        .cast(ConsensusMsg::<Ctx>::BlockReceived(value_assembled))
                        .unwrap();
                }
            }

            Msg::GetReceivedValue {
                height,
                round,
                reply,
            } => {
                let value = self
                    .value_builder
                    .maybe_received_value(height, round, &mut state.part_store)
                    .await;
                reply.send(value)?;
            }
        }

        Ok(())
    }
}
