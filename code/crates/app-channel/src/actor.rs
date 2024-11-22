use crate::channel::ChannelMsg;
use malachite_actors::host::HostMsg;
use malachite_common::Context;
use malachite_metrics::Metrics;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort, SpawnErr};
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::Sender as OneShotSender;

pub struct Connector<Ctx>
where
    Ctx: Context,
{
    sender: Sender<ChannelMsg<Ctx>>,
    // Todo: add some metrics
    #[allow(dead_code)]
    metrics: Metrics,
}

impl<Ctx> Connector<Ctx>
where
    Ctx: Context,
{
    pub fn new(sender: Sender<ChannelMsg<Ctx>>, metrics: Metrics) -> Self {
        Connector { sender, metrics }
    }

    pub async fn spawn(
        sender: Sender<ChannelMsg<Ctx>>,
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
            HostMsg::StartRound {
                height,
                round,
                proposer,
            } => {
                self.sender
                    .send(ChannelMsg::StartRound {
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
                let reply_to = create_reply_channel(reply_to).await?;

                self.sender
                    .send(ChannelMsg::GetValue {
                        height,
                        round,
                        timeout_duration,
                        address,
                        reply_to,
                    })
                    .await?
            }
            HostMsg::RestreamValue {
                height,
                round,
                valid_round,
                address,
                value_id,
            } => {
                self.sender
                    .send(ChannelMsg::RestreamValue {
                        height,
                        round,
                        valid_round,
                        address,
                        value_id,
                    })
                    .await?
            }
            HostMsg::GetEarliestBlockHeight { reply_to } => {
                let reply_to = create_reply_channel(reply_to).await?;
                self.sender
                    .send(ChannelMsg::GetEarliestBlockHeight { reply_to })
                    .await?;
            }
            HostMsg::ReceivedProposalPart {
                from,
                part,
                reply_to,
            } => {
                let reply_to = create_reply_channel(reply_to).await?;
                self.sender
                    .send(ChannelMsg::ReceivedProposalPart {
                        from,
                        part,
                        reply_to,
                    })
                    .await?;
            }
            HostMsg::GetValidatorSet { height, reply_to } => {
                let reply_to = create_reply_channel(reply_to).await?;
                self.sender
                    .send(ChannelMsg::GetValidatorSet { height, reply_to })
                    .await?;
            }
            HostMsg::Decide { certificate, .. } => {
                self.sender.send(ChannelMsg::Decide { certificate }).await?
            }
            HostMsg::GetDecidedBlock { height, reply_to } => {
                let reply_to = create_reply_channel(reply_to).await?;
                self.sender
                    .send(ChannelMsg::GetDecidedBlock { height, reply_to })
                    .await?;
            }
            HostMsg::ProcessSyncedBlockBytes {
                height,
                round,
                validator_address,
                block_bytes,
                reply_to,
            } => {
                let reply_to = create_reply_channel(reply_to).await?;
                self.sender
                    .send(ChannelMsg::ProcessSyncedBlockBytes {
                        height,
                        round,
                        validator_address,
                        block_bytes,
                        reply_to,
                    })
                    .await?;
            }
        };
        Ok(())
    }
}

async fn create_reply_channel<T>(
    reply_to: RpcReplyPort<T>,
) -> Result<OneShotSender<T>, ActorProcessingErr>
where
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<T>();

    tokio::spawn(async move { reply_to.send(rx.await.unwrap()) }).await??;

    Ok(tx)
}
