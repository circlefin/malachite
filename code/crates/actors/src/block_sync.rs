use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::time::Duration;

use async_trait::async_trait;
use derive_where::derive_where;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::task::JoinHandle;
use tracing::{info, trace};

use malachite_common::Context;
use malachite_gossip_consensus::PeerId;

use crate::gossip_consensus::{GossipConsensusMsg, GossipConsensusRef, GossipEvent, Status};
use crate::host::{Certificate, HostMsg, HostRef};
use crate::util::forward::forward;

pub type BlockSyncRef<Ctx> = ActorRef<Msg<Ctx>>;

#[derive_where(Clone, Debug)]
pub struct RawDecidedBlock<Ctx: Context> {
    pub certificate: Certificate<Ctx>,
    pub block_bytes: Vec<u8>,
}

#[derive_where(Clone, Debug)]
pub enum Msg<Ctx: Context> {
    Tick,
    GossipEvent(GossipEvent<Ctx>),

    // Consensus has decided on a value
    Decided { height: Ctx::Height },

    DecidedBlock(Option<RawDecidedBlock<Ctx>>),
}

#[derive_where(Clone, Debug, Default)]
struct BlockSyncState<Ctx>
where
    Ctx: Context,
{
    // Current Height
    current_height: Ctx::Height,

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

    #[tracing::instrument(name = "blocksync", skip_all)]
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        #[allow(clippy::single_match)]
        match msg {
            Msg::GossipEvent(event) => {
                if let GossipEvent::Status(p, ref status) = event {
                    trace!("SYNC Received Status event: {event:?}");
                    state.blocksync.store_peer_height(p, status.height);
                    if status.height < state.blocksync.current_height {
                        info!(
                            "SYNC REQUIRED peer falling behind {p} at {}, my height {}",
                            status.height, state.blocksync.current_height
                        );

                        // Retrieve the block for status.height
                        self.host.call_and_forward(
                            |reply| HostMsg::DecidedBlock {
                                height: status.height,
                                reply_to: reply,
                            },
                            &myself,
                            |decided_block: Option<RawDecidedBlock<Ctx>>| {
                                crate::block_sync::Msg::<Ctx>::DecidedBlock(decided_block)
                            },
                            None,
                        )?;
                    }
                }
            }

            Msg::Decided { height, .. } => {
                state.blocksync.current_height = height;
            }

            Msg::Tick => {
                let status = Status {
                    height: state.blocksync.current_height,
                };

                self.gossip_consensus
                    .cast(GossipConsensusMsg::PublishStatus(status))?;
            }

            Msg::DecidedBlock(_decided_block) => {
                // TODO - send this as a response in the blocksync request response
            }
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
