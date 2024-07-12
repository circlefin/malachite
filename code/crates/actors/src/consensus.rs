use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef};
use tokio::sync::mpsc;
use tracing::{error, info};

use malachite_common::{Context, Round, Timeout};
use malachite_consensus::{Effect, GossipMsg, Resume};
use malachite_driver::Driver;
use malachite_gossip_consensus::{Channel, Event as GossipEvent};
use malachite_metrics::Metrics;
use malachite_vote::ThresholdParams;

use crate::gossip_consensus::{GossipConsensusRef, Msg as GossipConsensusMsg};
use crate::host::{HostMsg, HostRef, LocallyProposedValue, ReceivedProposedValue};
use crate::timers::{Config as TimersConfig, Msg as TimersMsg, TimeoutElapsed, Timers, TimersRef};
use crate::util::forward;

pub struct ConsensusParams<Ctx: Context> {
    pub start_height: Ctx::Height,
    pub initial_validator_set: Ctx::ValidatorSet,
    pub address: Ctx::Address,
    pub threshold_params: ThresholdParams,
}

pub type ConsensusRef<Ctx> = ActorRef<Msg<Ctx>>;

pub type TxDecision<Ctx> = mpsc::Sender<(<Ctx as Context>::Height, Round, <Ctx as Context>::Value)>;

pub struct Consensus<Ctx>
where
    Ctx: Context,
{
    ctx: Ctx,
    params: ConsensusParams<Ctx>,
    timers_config: TimersConfig,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    metrics: Metrics,
    tx_decision: Option<TxDecision<Ctx>>,
}

pub type ConsensusMsg<Ctx> = Msg<Ctx>;

pub enum Msg<Ctx: Context> {
    GossipEvent(Arc<GossipEvent<Ctx>>),
    TimeoutElapsed(Timeout),
    // The proposal builder has built a value and can be used in a new proposal consensus message
    ProposeValue(Ctx::Height, Round, Ctx::Value),
    // The proposal builder has build a new block part, needs to be signed and gossiped by consensus
    GossipBlockPart(Ctx::BlockPart),
    BlockReceived(ReceivedProposedValue<Ctx>),
}

type InnerMsg<Ctx> = malachite_consensus::Msg<Ctx>;

impl<Ctx: Context> From<TimeoutElapsed> for Msg<Ctx> {
    fn from(msg: TimeoutElapsed) -> Self {
        Msg::TimeoutElapsed(msg.timeout())
    }
}

pub struct State<Ctx: Context> {
    timers: TimersRef,
    consensus: malachite_consensus::State<Ctx>,
}

impl<Ctx> Consensus<Ctx>
where
    Ctx: Context,
{
    pub fn new(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timers_config: TimersConfig,
        gossip_consensus: GossipConsensusRef<Ctx>,
        host: HostRef<Ctx>,
        metrics: Metrics,
        tx_decision: Option<TxDecision<Ctx>>,
    ) -> Self {
        Self {
            ctx,
            params,
            timers_config,
            gossip_consensus,
            host,
            metrics,
            tx_decision,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        ctx: Ctx,
        params: ConsensusParams<Ctx>,
        timers_config: TimersConfig,
        gossip_consensus: GossipConsensusRef<Ctx>,
        host: HostRef<Ctx>,
        metrics: Metrics,
        tx_decision: Option<TxDecision<Ctx>>,
        supervisor: Option<ActorCell>,
    ) -> Result<ActorRef<Msg<Ctx>>, ractor::SpawnErr> {
        let node = Self::new(
            ctx,
            params,
            timers_config,
            gossip_consensus,
            host,
            metrics,
            tx_decision,
        );

        let (actor_ref, _) = if let Some(supervisor) = supervisor {
            Actor::spawn_linked(None, node, (), supervisor).await?
        } else {
            Actor::spawn(None, node, ()).await?
        };

        Ok(actor_ref)
    }

    async fn process_msg(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        msg: InnerMsg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        malachite_consensus::process!(
            msg: msg,
            state: &mut state.consensus,
            metrics: &self.metrics,
            with: effect => self.handle_effect(myself, &state.timers, effect).await
        )
    }

    async fn handle_msg(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        msg: Msg<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::ProposeValue(height, round, value) => {
                let result = self
                    .process_msg(&myself, state, InnerMsg::ProposeValue(height, round, value))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing ProposeValue message: {e:?}");
                }

                Ok(())
            }

            Msg::GossipEvent(event) => {
                let result = self
                    .process_msg(
                        &myself,
                        state,
                        InnerMsg::GossipEvent(Arc::unwrap_or_clone(event)),
                    )
                    .await;

                if let Err(e) = result {
                    error!("Error when processing GossipEvent message: {e:?}");
                }

                Ok(())
            }

            Msg::TimeoutElapsed(timeout) => {
                let result = self
                    .process_msg(&myself, state, InnerMsg::TimeoutElapsed(timeout))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing TimeoutElapsed message: {e:?}");
                }

                Ok(())
            }

            Msg::BlockReceived(block) => {
                let result = self
                    .process_msg(&myself, state, InnerMsg::BlockReceived(block))
                    .await;

                if let Err(e) = result {
                    error!("Error when processing GossipEvent message: {e:?}");
                }

                Ok(())
            }

            Msg::GossipBlockPart(block_part) => {
                let signed_block_part = self.ctx.sign_block_part(block_part);
                let gossip_msg = GossipConsensusMsg::Broadcast(
                    Channel::BlockParts,
                    GossipMsg::BlockPart(signed_block_part),
                );

                if let Err(e) = self.gossip_consensus.cast(gossip_msg) {
                    error!("Error when sending block part to gossip layer: {e:?}");
                }

                Ok(())
            }
        }
    }

    #[tracing::instrument(skip(self, myself))]
    fn get_value(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        height: Ctx::Height,
        round: Round,
        timeout: Timeout,
    ) -> Result<(), ActorProcessingErr> {
        let timeout_duration = self.timers_config.timeout_duration(timeout.step);

        // Call `GetValue` on the Host actor, and forward the reply
        // to the current actor, wrapping it in `Msg::ProposeValue`.
        self.host.call_and_forward(
            |reply| HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                address: self.params.address.clone(),
                consensus: myself.clone(),
                reply_to: reply,
            },
            myself,
            |proposed: LocallyProposedValue<Ctx>| {
                Msg::<Ctx>::ProposeValue(proposed.height, proposed.round, proposed.value)
            },
            None,
        )?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_validator_set(
        &self,
        height: Ctx::Height,
    ) -> Result<Ctx::ValidatorSet, ActorProcessingErr> {
        let validator_set = ractor::call!(self.host, |reply_to| HostMsg::GetValidatorSet {
            height,
            reply_to
        })
        .map_err(|e| format!("Error at height {height} when waiting for validator set: {e:?}"))?;

        Ok(validator_set)
    }

    #[tracing::instrument(skip_all)]
    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        timers: &TimersRef,
        effect: Effect<Ctx>,
    ) -> Result<Resume<Ctx>, ActorProcessingErr> {
        match effect {
            Effect::ResetTimeouts => {
                timers
                    .cast(TimersMsg::ResetTimeouts)
                    .map_err(|e| format!("Error when resetting timeouts: {e:?}"))?;

                Ok(Resume::Continue)
            }
            Effect::CancelAllTimeouts => {
                timers
                    .cast(TimersMsg::CancelAllTimeouts)
                    .map_err(|e| format!("Error when cancelling all timeouts: {e:?}"))?;

                Ok(Resume::Continue)
            }
            Effect::CancelTimeout(timeout) => {
                timers
                    .cast(TimersMsg::CancelTimeout(timeout))
                    .map_err(|e| format!("Error when cancelling timeout {timeout}: {e:?}"))?;

                Ok(Resume::Continue)
            }
            Effect::ScheduleTimeout(timeout) => {
                timers
                    .cast(TimersMsg::ScheduleTimeout(timeout))
                    .map_err(|e| format!("Error when scheduling timeout {timeout}: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::Broadcast(gossip_msg) => {
                self.gossip_consensus
                    .cast(GossipConsensusMsg::Broadcast(
                        Channel::Consensus,
                        gossip_msg,
                    ))
                    .map_err(|e| format!("Error when broadcasting gossip message: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::GetValue(height, round, timeout) => {
                if let Err(e) = self.get_value(myself, height, round, timeout) {
                    error!("Error when asking for value to be built: {e:?}");
                }

                Ok(Resume::Continue)
            }

            Effect::GetValidatorSet(height) => {
                let validator_set = self.get_validator_set(height).await.map_err(|e| {
                    format!("Error when getting validator set at height {height}: {e:?}")
                })?;

                Ok(Resume::ValidatorSet(height, validator_set))
            }

            Effect::DecidedOnValue {
                height,
                round,
                value,
                commits,
            } => {
                if let Some(tx_decision) = &self.tx_decision {
                    let _ = tx_decision.send((height, round, value.clone())).await;
                }

                self.host
                    .cast(HostMsg::DecidedOnValue {
                        height,
                        round,
                        value,
                        commits,
                    })
                    .map_err(|e| format!("Error when sending decided value to host: {e:?}"))?;

                Ok(Resume::Continue)
            }

            Effect::ReceivedBlockPart(block_part) => {
                self.host
                    .call_and_forward(
                        |reply_to| HostMsg::ReceivedBlockPart {
                            block_part,
                            reply_to,
                        },
                        myself,
                        |value| Msg::BlockReceived(value),
                        None,
                    )
                    .map_err(|e| format!("Error when forwarding block part to host: {e:?}"))?;

                Ok(Resume::Continue)
            }
        }
    }
}

#[async_trait]
impl<Ctx> Actor for Consensus<Ctx>
where
    Ctx: Context,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = ();

    #[tracing::instrument(name = "consensus", skip_all)]
    async fn pre_start(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        _args: (),
    ) -> Result<State<Ctx>, ActorProcessingErr> {
        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;

        self.gossip_consensus
            .cast(GossipConsensusMsg::Subscribe(forward))?;

        let (timers, _) =
            Timers::spawn_linked(self.timers_config, myself.clone(), myself.get_cell()).await?;

        let driver = Driver::new(
            self.ctx.clone(),
            self.params.start_height,
            self.params.initial_validator_set.clone(),
            self.params.address.clone(),
            self.params.threshold_params,
        );

        let consensus_state = malachite_consensus::State {
            ctx: self.ctx.clone(),
            driver,
            msg_queue: VecDeque::new(),
            connected_peers: BTreeSet::new(),
            received_blocks: vec![],
            signed_precommits: Default::default(),
        };

        Ok(State {
            timers,
            consensus: consensus_state,
        })
    }

    #[tracing::instrument(
        name = "consensus",
        skip_all,
        fields(
            height = %state.consensus.driver.height(),
            round = %state.consensus.driver.round()
        )
    )]
    async fn handle(
        &self,
        myself: ActorRef<Msg<Ctx>>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        self.handle_msg(myself, state, msg).await
    }

    #[tracing::instrument(
        name = "consensus",
        skip_all,
        fields(
            height = %state.consensus.driver.height(),
            round = %state.consensus.driver.round()
        )
    )]
    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        info!("Stopping...");

        state.timers.stop(None);

        Ok(())
    }
}
