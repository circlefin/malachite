use crate::channel::{AppMsg, ConsensusMsg};
use malachite_actors::consensus::Msg;
use malachite_actors::host::HostMsg;
use malachite_common::Context;
use malachite_metrics::Metrics;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, SpawnErr};
use tokio::sync::mpsc::Sender;

pub struct Connector<Ctx>
where
    Ctx: Context,
{
    sender: Sender<AppMsg<Ctx>>,
    // Todo: add some metrics
    #[allow(dead_code)]
    metrics: Metrics,
}

impl<Ctx> Connector<Ctx>
where
    Ctx: Context,
{
    pub fn new(sender: Sender<AppMsg<Ctx>>, metrics: Metrics) -> Self {
        Connector { sender, metrics }
    }

    pub async fn spawn(
        sender: Sender<AppMsg<Ctx>>,
        metrics: Metrics,
    ) -> Result<ActorRef<HostMsg<Ctx>>, SpawnErr>
    where
        Ctx: Context,
    {
        let (actor_ref, _) = Actor::spawn(None, Self::new(sender, metrics), ()).await?;
        Ok(actor_ref)
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
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            HostMsg::ConsensusReady(consensus_ref) => {
                let (tx, rx) = tokio::sync::oneshot::channel();

                self.sender
                    .send(AppMsg::ConsensusReady { reply_to: tx })
                    .await?;

                consensus_ref.cast(translate_consensus_msg(rx.await?))?;
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
                timeout_duration,
                address,
                reply_to,
            } => {
                let (tx, rx) = tokio::sync::oneshot::channel();

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
                let (tx, rx) = tokio::sync::oneshot::channel();

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
                let (tx, rx) = tokio::sync::oneshot::channel();

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
                let (tx, rx) = tokio::sync::oneshot::channel();

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
                let (tx, rx) = tokio::sync::oneshot::channel();

                self.sender
                    .send(AppMsg::Decided {
                        certificate,
                        reply_to: tx,
                    })
                    .await?;

                consensus_ref.cast(translate_consensus_msg(rx.await?))?;
            }
            HostMsg::GetDecidedBlock { height, reply_to } => {
                let (tx, rx) = tokio::sync::oneshot::channel();

                self.sender
                    .send(AppMsg::GetDecidedBlock {
                        height,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }
            HostMsg::ProcessSyncedBlock {
                height,
                round,
                validator_address,
                block_bytes,
                reply_to,
            } => {
                let (tx, rx) = tokio::sync::oneshot::channel();

                self.sender
                    .send(AppMsg::ProcessSyncedBlock {
                        height,
                        round,
                        validator_address,
                        block_bytes,
                        reply_to: tx,
                    })
                    .await?;

                reply_to.send(rx.await?)?;
            }
        };
        Ok(())
    }
}

fn translate_consensus_msg<Ctx>(consensus_msg: ConsensusMsg<Ctx>) -> Msg<Ctx>
where
    Ctx: Context + Send + 'static,
{
    match consensus_msg {
        ConsensusMsg::StartHeight(height) => Msg::StartHeight(height),
    }
}
