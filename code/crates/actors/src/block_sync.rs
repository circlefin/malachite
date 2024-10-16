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
use malachite_common::{Certificate, Context, Height, InclusiveRange, Proposal};

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

    /// Consensus has decided on a value
    Decided { height: Ctx::Height },

    /// Consensus has started a new height
    StartHeight { height: Ctx::Height },

    /// Host has a response for the blocks request
    DecidedBlocks(InboundRequestId, Vec<SyncedBlock<Ctx>>),
}

#[derive(Debug)]
pub struct Args {
    pub max_batch_size: u64,
    pub status_update_interval: Duration,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            max_batch_size: 10,
            status_update_interval: Duration::from_secs(10),
        }
    }
}

#[derive_where(Debug)]
pub struct State<Ctx: Context> {
    /// The state of the blocksync state machine
    blocksync: blocksync::State<Ctx>,
    /// Maximum number of blocks to request at once from a peer
    max_batch_size: u64,
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
            max_batch_size: args.max_batch_size,
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

                    // If there are no pending requests then ask for block from peer
                    if !state.blocksync.pending_requests.contains_key(&sync_height) {
                        let max_height =
                            peer_height.min(sync_height.increment_by(state.max_batch_size - 1));

                        let heights = InclusiveRange::from(sync_height..=max_height);

                        debug!(%heights, %peer, "Requesting blocks from peer");

                        self.gossip
                            .cast(OutgoingBlockSyncRequest(peer, Request::new(heights)))?;

                        state.blocksync.store_pending_request(heights, peer);
                    }
                }
            }

            Msg::GossipEvent(GossipEvent::BlockSyncRequest(
                request_id,
                blocksync::Request { heights },
            )) => {
                debug!(%heights, "Received request for blocks");

                // Retrieve the blocks for the requested heights
                self.host.call_and_forward(
                    |reply_to| HostMsg::GetDecidedBlocks { heights, reply_to },
                    &myself,
                    move |blocks| Msg::<Ctx>::DecidedBlocks(request_id, blocks),
                    None,
                )?;
            }

            Msg::Decided { height, .. } => {
                debug!(%height, "Decided height");

                state.blocksync.tip_height = height;
                state.blocksync.remove_pending_request(height);
            }

            Msg::StartHeight { height } => {
                debug!(%height, "Starting new height");

                state.blocksync.sync_height = height;

                for (peer, &peer_height) in &state.blocksync.peers {
                    if peer_height > height {
                        let max_height =
                            peer_height.min(height.increment_by(state.max_batch_size - 1));

                        let heights = InclusiveRange::from(height..=max_height);

                        debug!(
                            %heights,
                            %peer_height,
                            %peer,
                            "Starting new height, requesting blocks"
                        );

                        self.gossip
                            .cast(OutgoingBlockSyncRequest(*peer, Request { heights }))?;

                        state.blocksync.store_pending_request(heights, *peer);

                        break;
                    }
                }
            }

            Msg::DecidedBlocks(request_id, decided_blocks) if !decided_blocks.is_empty() => {
                let heights = InclusiveRange::new(
                    decided_blocks.first().as_ref().unwrap().proposal.height(),
                    decided_blocks.last().as_ref().unwrap().proposal.height(),
                );

                debug!(%heights, "Received decided blocks");

                self.gossip
                    .cast(GossipConsensusMsg::OutgoingBlockSyncResponse(
                        request_id,
                        decided_blocks,
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
