use std::fmt::Debug;
use std::time::Duration;
use tracing::info;

use crate::util::value_builder::test::PartStore;
use malachite_common::{Context, Round};
use malachite_driver::Validity;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::util::ValueBuilder;

#[derive(Debug)]
pub struct LocallyProposedValue<Ctx: Context> {
    pub height: Ctx::Height,
    pub round: Round,
    pub value: Option<Ctx::Value>, // todo - should we remove?
}

#[derive(Debug)]
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
        gossip_actor: ActorRef<crate::consensus::Msg<Ctx>>,
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
    gossip_actor: Option<ActorRef<crate::consensus::Msg<Ctx>>>,
    part_store: PartStore,
}

pub struct ProposalBuilder<Ctx: Context + std::fmt::Debug> {
    #[allow(dead_code)]
    ctx: Ctx,
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
        if value.is_some() {
            info!(
                "Value Builder received all parts, produced value {:?} for proposal",
                value
            );
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
            gossip_actor: None,
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
                gossip_actor,
                part_store,
            } => {
                state.gossip_actor = Some(gossip_actor);
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
                        state.gossip_actor.clone(),
                        &mut state.part_store,
                    )
                    .await?;
                reply.send(value)?;
            }

            Msg::BlockPart(block_part) => {
                let maybe_block = self.build_value(block_part, &mut state.part_store).await?;
                // TODO - Send the proposed value (from blockparts) to Driver
                // to be maybe multiplexed with the proposal (from consensus)
                if let Some(value_assembled) = maybe_block {
                    state
                        .gossip_actor
                        .as_ref()
                        .unwrap()
                        .cast(crate::consensus::Msg::<Ctx>::BlockReceived(value_assembled))
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
