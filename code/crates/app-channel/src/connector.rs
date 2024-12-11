//! Implementation of a host actor acting as a bridge between consensus and the application.

use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, SpawnErr};
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use malachite_actors::host::HostMsg;

use crate::app::types::core::Context;
use crate::app::types::metrics::Metrics;
use crate::channel::AppMsg;

pub struct Connector<Ctx>
where
    Ctx: Context,
{
    sender: mpsc::Sender<AppMsg<Ctx>>,

    // TODO: add some metrics
    #[allow(dead_code)]
    metrics: Metrics,
}

impl<Ctx> Connector<Ctx>
where
    Ctx: Context,
{
    pub fn new(sender: mpsc::Sender<AppMsg<Ctx>>, metrics: Metrics) -> Self {
        Connector { sender, metrics }
    }

    pub async fn spawn(
        sender: mpsc::Sender<AppMsg<Ctx>>,
        metrics: Metrics,
    ) -> Result<ActorRef<HostMsg<Ctx>>, SpawnErr>
    where
        Ctx: Context,
    {
        let (actor_ref, _) = Actor::spawn(None, Self::new(sender, metrics), ()).await?;
        Ok(actor_ref)
    }
}

impl<Ctx> Connector<Ctx>
where
    Ctx: Context,
{
    async fn handle_msg(
        &self,
        _myself: ActorRef<HostMsg<Ctx>>,
        msg: HostMsg<Ctx>,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::ConsensusReady(consensus_ref) => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::ConsensusReady { reply_to: tx })
                    .await?;

                consensus_ref.cast(rx.await?.into())?;
            }

            HostMsg::StartedRound {
                height,
                round,
                proposer,
            } => {
                self.sender
                    .send(AppMsg::StartedRound {
                        height,
                        round,
                        proposer,
                    })
                    .await?
            }

            HostMsg::GetValue {
                height,
                round,
                timeout: timeout_duration,
                address,
                reply_to,
            } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::GetValue {
                        height,
                        round,
                        timeout_duration,
                        address,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }

            HostMsg::RestreamValue {
                height,
                round,
                valid_round,
                address,
                value_id,
            } => {
                self.sender
                    .send(AppMsg::RestreamValue {
                        height,
                        round,
                        valid_round,
                        address,
                        value_id,
                    })
                    .await?
            }

            HostMsg::GetEarliestBlockHeight { reply_to } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::GetEarliestBlockHeight { reply_to: tx })
                    .await?;

                reply_to.send(rx.await?)?;
            }

            HostMsg::ReceivedProposalPart {
                from,
                part,
                reply_to,
            } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::ReceivedProposalPart {
                        from,
                        part,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }

            HostMsg::GetValidatorSet { height, reply_to } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::GetValidatorSet {
                        height,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }

            HostMsg::Decided {
                certificate,
                consensus: consensus_ref,
            } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::Decided {
                        certificate,
                        reply_to: tx,
                    })
                    .await?;

                consensus_ref.cast(rx.await?.into())?;
            }

            HostMsg::GetDecidedValue { height, reply_to } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::GetDecidedBlock {
                        height,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }

            HostMsg::ProcessSyncedValue {
                height,
                round,
                validator_address,
                value_bytes,
                reply_to,
            } => {
                let (tx, rx) = oneshot::channel();

                self.sender
                    .send(AppMsg::ProcessSyncedValue {
                        height,
                        round,
                        validator_address,
                        value_bytes,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }
        };

        Ok(())
    }
}

#[async_trait]
impl<Ctx> Actor for Connector<Ctx>
where
    Ctx: Context,
{
    type Msg = HostMsg<Ctx>;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(())
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(e) = self.handle_msg(myself, msg, state).await {
            tracing::error!("Error processing message: {e}");
        }

        Ok(())
    }
}
