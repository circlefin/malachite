use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use derive_where::derive_where;
use libp2p::request_response::InboundRequestId;
use malachite_blocksync::{Request, SyncedBlock};
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::task::JoinHandle;
use tracing::{debug, info};

use malachite_common::{Certificate, Context, Proposal};
use malachite_gossip_consensus::PeerId;

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
    Tick,
    GossipEvent(GossipEvent<Ctx>),

    // Consensus has decided on a value
    Decided { height: Ctx::Height },

    // Consensus has started a new height
    StartHeight { height: Ctx::Height },

    // Host has a response for the block request
    DecidedBlock(InboundRequestId, Option<SyncedBlock<Ctx>>),
}

#[derive_where(Clone, Debug, Default)]
struct BlockSyncState<Ctx>
where
    Ctx: Context,
{
    // Height of last decided block
    tip_height: Ctx::Height,

    // Height currently syncing.
    sync_height: Ctx::Height,

    // Requests for these heights have been sent out to peers.
    pending_requests: BTreeMap<Ctx::Height, PeerId>,

    // The set of peers we are connected to in order to get blocks and certificates.
    peers: BTreeMap<PeerId, Ctx::Height>,
}

impl<Ctx> BlockSyncState<Ctx>
where
    Ctx: Context,
{
    pub fn store_peer_height(&mut self, peer: PeerId, height: Ctx::Height) {
        self.peers.insert(peer, height);
    }
    pub fn store_pending_request(&mut self, height: Ctx::Height, peer: PeerId) {
        self.pending_requests.insert(height, peer);
    }
    pub fn remove_pending_request(&mut self, height: Ctx::Height) {
        self.pending_requests.remove(&height);
    }
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
    blocksync: BlockSyncState<Ctx>,
    ticker: JoinHandle<()>,
    marker: PhantomData<Ctx>,
}

#[allow(dead_code)]
pub struct BlockSync<Ctx: Context> {
    ctx: Ctx,
    gossip_consensus: GossipConsensusRef<Ctx>,
    host: HostRef<Ctx>,
}

impl<Ctx> BlockSync<Ctx>
where
    Ctx: Context,
{
    pub fn new(ctx: Ctx, gossip_consensus: GossipConsensusRef<Ctx>, host: HostRef<Ctx>) -> Self {
        Self {
            ctx,
            gossip_consensus,
            host,
        }
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

        self.gossip_consensus
            .cast(GossipConsensusMsg::Subscribe(forward))?;

        let ticker = tokio::spawn(async move {
            loop {
                tokio::time::sleep(args.status_update_interval).await;

                if let Err(e) = myself.cast(Msg::Tick) {
                    tracing::error!(?e, "Failed to send tick message");
                }
            }
        });

        Ok(State {
            blocksync: BlockSyncState::default(),
            ticker,
            marker: PhantomData,
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
        #[allow(clippy::single_match)]
        match msg {
            Msg::GossipEvent(GossipEvent::Status(peer, ref status)) => {
                let peer_height = status.height;
                debug!("Received Status {:?} from peer {peer}", status);
                state.blocksync.store_peer_height(peer, status.height);
                if status.height > state.blocksync.tip_height {
                    info!(
                        "SYNC REQUIRED: falling behind {peer} at {}, my height {}",
                        status.height, state.blocksync.tip_height
                    );
                }
                let height = state.blocksync.sync_height;
                // If there are no pending requests then ask for block from peer
                if !state.blocksync.pending_requests.contains_key(&height) {
                    debug!("Requesting block {height} from {peer:?} that is at {peer_height:?}");
                    self.gossip_consensus
                        .cast(OutgoingBlockSyncRequest(peer, Request { height }))?;
                    state.blocksync.store_pending_request(height, peer);
                }
            }

            Msg::GossipEvent(GossipEvent::BlockSyncRequest(request_id, request)) => {
                debug!("Received request for block height {}", request.height);
                // Retrieve the block for request.height
                self.host.call_and_forward(
                    |reply| HostMsg::DecidedBlock {
                        height: request.height,
                        reply_to: reply,
                    },
                    &myself,
                    move |decided_block: Option<SyncedBlock<Ctx>>| {
                        Msg::<Ctx>::DecidedBlock(request_id, decided_block)
                    },
                    None,
                )?;
            }

            Msg::Decided { height, .. } => {
                debug!("Decided height {height}");
                state.blocksync.tip_height = height;
                state.blocksync.remove_pending_request(height);
            }

            Msg::StartHeight { height } => {
                state.blocksync.sync_height = height;
                debug!("Starting new height {height}");
                for (peer, peer_height) in state.blocksync.peers.iter() {
                    if *peer_height > height {
                        debug!(
                            "Starting new height {height}, requesting the block from {peer:?} that is at {peer_height:?}"
                        );
                        self.gossip_consensus
                            .cast(OutgoingBlockSyncRequest(*peer, Request { height }))?;
                        state.blocksync.store_pending_request(height, *peer);

                        break;
                    }
                }
            }

            Msg::Tick => {
                let status = Status {
                    height: state.blocksync.tip_height,
                };

                self.gossip_consensus
                    .cast(GossipConsensusMsg::PublishStatus(status))?;
            }

            Msg::DecidedBlock(request_id, Some(decided_block)) => {
                debug!(
                    "Received decided block for {}",
                    decided_block.proposal.height()
                );
                self.gossip_consensus
                    .cast(GossipConsensusMsg::OutgoingBlockSyncResponse(
                        request_id,
                        decided_block,
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
