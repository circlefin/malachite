use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use derive_where::derive_where;
use libp2p::request_response::InboundRequestId;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::task::JoinHandle;
use tracing::{debug, error_span, info};

use malachite_blocksync as blocksync;
use malachite_blocksync::{Request, SyncedBlock};
use malachite_common::{Certificate, Context, Proposal};

use crate::gossip_consensus::Msg::OutgoingBlockSyncRequest;
use crate::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef, GossipEvent, Status};
use crate::host::{HostMsg, HostRef};
use crate::util::forward::forward;

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
    DecidedBlock(InboundRequestId, Option<SyncedBlock<Ctx>>),
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
}

impl<Ctx> BlockSync<Ctx>
where
    Ctx: Context,
{
    pub fn new(ctx: Ctx, gossip: GossipConsensusRef<Ctx>, host: HostRef<Ctx>) -> Self {
        Self { ctx, gossip, host }
    }

    pub async fn spawn(self) -> Result<(BlockSyncRef<Ctx>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, Args::default()).await
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

        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(args.status_update_interval).await;

                if let Err(e) = myself.cast(Msg::Tick) {
                    tracing::error!(?e, "Failed to send tick message");
                }
            }
        });

        Ok(State {
            blocksync: blocksync::State::default(),
            ticker,
        })
    }

    // TODO:
    //  - move to blocksync crate
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
                let status = Status {
                    height: state.blocksync.tip_height,
                };

                self.gossip
                    .cast(GossipConsensusMsg::PublishStatus(status))?;
            }

            Msg::GossipEvent(GossipEvent::Status(peer, status)) => {
                let peer_height = status.height;
                let sync_height = state.blocksync.sync_height;
                let tip_height = state.blocksync.tip_height;

                let _span = error_span!("status", %sync_height, %tip_height).entered();

                debug!(%peer_height, %peer, "Received peer status");

                state.blocksync.store_peer_height(peer, peer_height);

                if peer_height > tip_height {
                    info!(%peer_height, %peer, "SYNC REQUIRED: Falling behind");

                    // If there are no pending requests for the base height yet then ask for a batch of blocks from peer
                    if !state.blocksync.pending_requests.contains_key(&sync_height) {
                        debug!(%sync_height, %peer, "Requesting block from peer");

                        self.gossip
                            .cast(OutgoingBlockSyncRequest(peer, Request::new(sync_height)))?;

                        state.blocksync.store_pending_request(sync_height, peer);
                    }
                }
            }

            Msg::GossipEvent(GossipEvent::BlockSyncRequest(
                request_id,
                blocksync::Request { height },
            )) => {
                debug!(%height, "Received request for block");

                // Retrieve the blocks for the requested heights
                self.host.call_and_forward(
                    |reply_to| HostMsg::GetDecidedBlock { height, reply_to },
                    &myself,
                    move |block| Msg::<Ctx>::DecidedBlock(request_id, block),
                    None,
                )?;
            }

            Msg::Decided(height) => {
                debug!(%height, "Decided height");

                state.blocksync.tip_height = height;
                state.blocksync.remove_pending_request(height);
            }

            Msg::StartHeight(height) => {
                debug!(%height, "Starting new height");

                state.blocksync.sync_height = height;

                for (peer, &peer_height) in &state.blocksync.peers {
                    if peer_height > height {
                        debug!(%height, %peer_height, %peer, "Starting new height, requesting block");

                        self.gossip
                            .cast(OutgoingBlockSyncRequest(*peer, Request::new(height)))?;

                        state.blocksync.store_pending_request(height, *peer);

                        break;
                    }
                }
            }

            Msg::DecidedBlock(request_id, decided_block) => {
                match &decided_block {
                    None => debug!("Received empty response"),
                    Some(block) => {
                        debug!(height = %block.proposal.height(), "Received decided block")
                    }
                }

                self.gossip
                    .cast(GossipConsensusMsg::OutgoingBlockSyncResponse(
                        request_id,
                        blocksync::Response::new(decided_block),
                    ))?;
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
