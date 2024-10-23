use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use derive_where::derive_where;
use libp2p::request_response::InboundRequestId;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::task::JoinHandle;

use malachite_blocksync as blocksync;
use malachite_blocksync::{Request, SyncedBlock};
use malachite_common::{Certificate, Context};

use crate::gossip_consensus::Msg::OutgoingBlockSyncRequest;
use crate::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef, GossipEvent, Status};
use crate::host::{HostMsg, HostRef};
use crate::util::forward::forward;
use crate::util::ticker::ticker;

pub type BlockSyncRef<Ctx> = ActorRef<Msg<Ctx>>;

#[derive_where(Clone, Debug)]
pub struct RawDecidedBlock<Ctx: Context> {
    pub height: Ctx::Height,
    pub certificate: Certificate<Ctx>,
    pub block_bytes: Bytes,
}

#[derive_where(Clone, Debug)]
pub enum Msg<Ctx: Context> {
    /// Internal tick
    Tick,

    /// Receive an even from gossip layer
    GossipEvent(GossipEvent<Ctx>),

    /// Consensus has decided on a value at the given height
    Decided(Ctx::Height),

    /// Consensus has started a new height
    StartHeight(Ctx::Height),

    /// Host has a response for the blocks request
    GotDecidedBlock(Ctx::Height, InboundRequestId, Option<SyncedBlock<Ctx>>),
}

#[derive(Debug)]
pub struct Args {
    pub status_update_interval: Duration,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            status_update_interval: Duration::from_secs(10),
        }
    }
}

#[derive_where(Debug)]
pub struct State<Ctx: Context> {
    /// The state of the blocksync state machine
    blocksync: blocksync::State<Ctx>,
    /// Task for sending status updates
    ticker: JoinHandle<()>,
}

#[allow(dead_code)]
pub struct BlockSync<Ctx: Context> {
    ctx: Ctx,
    gossip: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
    metrics: blocksync::Metrics,
}

impl<Ctx> BlockSync<Ctx>
where
    Ctx: Context,
{
    pub fn new(ctx: Ctx, gossip: GossipConsensusRef<Ctx>, host: HostRef<Ctx>) -> Self {
        Self {
            ctx,
            gossip,
            host,
            metrics: blocksync::Metrics::default(),
        }
    }

    pub async fn spawn(self) -> Result<(BlockSyncRef<Ctx>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, Args::default()).await
    }

    async fn process_input(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        state: &mut State<Ctx>,
        input: blocksync::Input<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        malachite_blocksync::process!(
            input: input,
            state: &mut state.blocksync,
            metrics: &self.metrics,
            with: effect => {
                self.handle_effect(myself, effect).await
            }
        )
    }

    async fn handle_effect(
        &self,
        myself: &ActorRef<Msg<Ctx>>,
        effect: blocksync::Effect<Ctx>,
    ) -> Result<blocksync::Resume<Ctx>, ActorProcessingErr> {
        use blocksync::Effect;
        match effect {
            Effect::PublishStatus(height) => {
                self.gossip
                    .cast(GossipConsensusMsg::PublishStatus(Status::new(height)))?;
            }

            Effect::SendRequest(peer_id, request) => {
                self.gossip
                    .cast(OutgoingBlockSyncRequest(peer_id, request))?;
            }

            Effect::SendResponse(request_id, response) => {
                self.gossip
                    .cast(GossipConsensusMsg::OutgoingBlockSyncResponse(
                        request_id, response,
                    ))?;
            }

            Effect::GetBlock(request_id, height) => {
                self.host.call_and_forward(
                    |reply_to| HostMsg::GetDecidedBlock { height, reply_to },
                    myself,
                    move |block| Msg::<Ctx>::GotDecidedBlock(height, request_id, block),
                    None,
                )?;
            }
        }

        Ok(blocksync::Resume::default())
    }
}

#[async_trait]
impl<Ctx> Actor for BlockSync<Ctx>
where
    Ctx: Context,
{
    type Msg = Msg<Ctx>;
    type State = State<Ctx>;
    type Arguments = Args;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Args,
    ) -> Result<Self::State, ActorProcessingErr> {
        let forward = forward(myself.clone(), Some(myself.get_cell()), Msg::GossipEvent).await?;
        self.gossip.cast(GossipConsensusMsg::Subscribe(forward))?;

        let ticker = tokio::spawn(ticker(args.status_update_interval, myself.clone(), || {
            Msg::Tick
        }));

        Ok(State {
            blocksync: blocksync::State::default(),
            ticker,
        })
    }

    // TODO:
    //  - proper FSM
    //  - timeout requests
    //  - multiple requests for next few heights
    //  - etc
    #[tracing::instrument(name = "blocksync", skip_all)]
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::Tick => {
                self.process_input(&myself, state, blocksync::Input::Tick)
                    .await?;
            }

            Msg::GossipEvent(GossipEvent::Status(peer_id, status)) => {
                let status = blocksync::Status {
                    peer_id,
                    height: status.height,
                };

                self.process_input(&myself, state, blocksync::Input::Status(status))
                    .await?;
            }

            Msg::GossipEvent(GossipEvent::BlockSyncRequest(
                request_id,
                from,
                blocksync::Request { height },
            )) => {
                self.process_input(
                    &myself,
                    state,
                    blocksync::Input::Request(request_id, from, Request::new(height)),
                )
                .await?;
            }

            Msg::Decided(height) => {
                self.process_input(&myself, state, blocksync::Input::Decided(height))
                    .await?;
            }

            Msg::StartHeight(height) => {
                self.process_input(&myself, state, blocksync::Input::StartHeight(height))
                    .await?;
            }

            Msg::GotDecidedBlock(height, request_id, block) => {
                self.process_input(
                    &myself,
                    state,
                    blocksync::Input::GotBlock(request_id, height, block),
                )
                .await?;
            }

            _ => {}
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        state.ticker.abort();
        Ok(())
    }
}
