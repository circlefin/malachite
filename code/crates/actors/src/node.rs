use async_trait::async_trait;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SupervisionEvent};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use malachite_common::Context;

use crate::block_sync::BlockSyncRef;
use crate::consensus::ConsensusRef;
use crate::gossip_consensus::GossipConsensusRef;
use crate::gossip_mempool::GossipMempoolRef;
use crate::host::HostRef;

pub type NodeRef = ActorRef<()>;

#[allow(dead_code)]
pub struct Node<Ctx: Context> {
    ctx: Ctx,
    gossip_consensus: GossipConsensusRef<Ctx>,
    consensus: ConsensusRef<Ctx>,
    gossip_mempool: GossipMempoolRef,
    block_sync: Option<BlockSyncRef<Ctx>>,
    mempool: ActorCell,
    host: HostRef<Ctx>,
    start_height: Ctx::Height,
}

impl<Ctx> Node<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx,
        gossip_consensus: GossipConsensusRef<Ctx>,
        consensus: ConsensusRef<Ctx>,
        gossip_mempool: GossipMempoolRef,
        block_sync: Option<BlockSyncRef<Ctx>>,
        mempool: ActorCell,
        host: HostRef<Ctx>,
        start_height: Ctx::Height,
    ) -> Self {
        Self {
            ctx,
            gossip_consensus,
            consensus,
            gossip_mempool,
            block_sync,
            mempool,
            host,
            start_height,
        }
    }

    pub async fn spawn(self) -> Result<(ActorRef<()>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, ()).await
    }
}

#[async_trait]
impl<Ctx> Actor for Node<Ctx>
where
    Ctx: Context,
{
    type Msg = ();
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<(), ActorProcessingErr> {
        // Set ourselves as the supervisor of the other actors
        self.gossip_consensus.link(myself.get_cell());
        self.consensus.link(myself.get_cell());
        self.mempool.link(myself.get_cell());
        self.host.link(myself.get_cell());
        self.gossip_mempool.link(myself.get_cell());

        if let Some(actor) = &self.block_sync {
            actor.link(myself.get_cell());
        }

        Ok(())
    }

    #[tracing::instrument(name = "node", skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        _msg: Self::Msg,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    #[tracing::instrument(name = "node", skip_all)]
    async fn handle_supervisor_evt(
        &self,
        _myself: ActorRef<Self::Msg>,
        evt: SupervisionEvent,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        match evt {
            SupervisionEvent::ActorStarted(cell) => {
                info!(actor = %cell.get_id(), "Actor has started");
            }
            SupervisionEvent::ActorTerminated(cell, _state, reason) => {
                warn!(
                    "Actor {} has terminated: {}",
                    cell.get_id(),
                    reason.unwrap_or_default()
                );
            }
            SupervisionEvent::ActorFailed(cell, error) => {
                error!("Actor {} has failed: {error}", cell.get_id());
            }
            SupervisionEvent::ProcessGroupChanged(_) => (),
        }

        Ok(())
    }
}
