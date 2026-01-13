use async_trait::async_trait;
use ractor::{Actor, ActorId, ActorProcessingErr, ActorRef, SupervisionEvent};
use tokio::task::JoinHandle;
use tracing::{error, info};

use malachitebft_core_types::Context;

use crate::consensus::ConsensusRef;
use crate::host::HostRef;
use crate::network::NetworkRef;
use crate::sync::SyncRef;
use crate::wal::WalRef;

pub type NodeRef = ActorRef<()>;

#[allow(dead_code)]
pub struct Node<Ctx: Context> {
    ctx: Ctx,
    network: NetworkRef<Ctx>,
    consensus: ConsensusRef<Ctx>,
    wal: WalRef<Ctx>,
    sync: Option<SyncRef<Ctx>>,
    host: HostRef<Ctx>,
    span: tracing::Span,
}

impl<Ctx> Node<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        ctx: Ctx,
        network: NetworkRef<Ctx>,
        consensus: ConsensusRef<Ctx>,
        wal: WalRef<Ctx>,
        sync: Option<SyncRef<Ctx>>,
        host: HostRef<Ctx>,
        span: tracing::Span,
    ) -> Self {
        Self {
            ctx,
            network,
            consensus,
            wal,
            sync,
            host,
            span,
        }
    }

    pub async fn spawn(self) -> Result<(ActorRef<()>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(None, self, ()).await
    }

    fn should_shutdown_on_child_failure(&self, child_id: ActorId) -> ShutdownPolicy {
        if child_id == self.wal.get_id() {
            ShutdownPolicy::Shutdown(format!("WAL actor {child_id} has failed"))
        } else {
            ShutdownPolicy::Continue
        }
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
        self.network.link(myself.get_cell());
        self.consensus.link(myself.get_cell());
        self.host.link(myself.get_cell());
        self.wal.link(myself.get_cell());

        if let Some(actor) = &self.sync {
            actor.link(myself.get_cell());
        }

        Ok(())
    }

    #[tracing::instrument(name = "node", parent = &self.span, skip_all)]
    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        _msg: Self::Msg,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        Ok(())
    }

    #[tracing::instrument(name = "node", parent = &self.span, skip_all)]
    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        evt: SupervisionEvent,
        _state: &mut (),
    ) -> Result<(), ActorProcessingErr> {
        match evt {
            SupervisionEvent::ActorStarted(cell) => {
                info!(actor = %cell.get_id(), "Actor has started");
            }

            SupervisionEvent::ActorTerminated(cell, _state, reason) => {
                error!(
                    actor = %cell.get_id(),
                    "Actor has terminated: {}",
                    reason.unwrap_or_else(|| "no reason provided".to_string())
                );

                match self.should_shutdown_on_child_failure(cell.get_id()) {
                    ShutdownPolicy::Shutdown(reason) => {
                        error!("Shutting down node due to child actor termination: {reason}");
                        myself.stop(Some(format!(
                            "Shutting down node due to child actor termination: {reason}"
                        )));
                    }
                    ShutdownPolicy::Continue => {
                        info!("Continuing operation despite child actor termination");
                    }
                }
            }

            SupervisionEvent::ActorFailed(cell, error) => {
                error!(actor = %cell.get_id(), "Actor has failed: {error}");
                error!("Shutting down node due to child actor failure");

                match self.should_shutdown_on_child_failure(cell.get_id()) {
                    ShutdownPolicy::Shutdown(reason) => {
                        error!("Shutting down node due to child actor failure: {reason}");
                        myself.stop(Some(format!(
                            "Shutting down node due to child actor failure: {reason}"
                        )));
                    }
                    ShutdownPolicy::Continue => {
                        info!("Continuing operation despite child actor failure");
                    }
                }
            }

            SupervisionEvent::ProcessGroupChanged(_) => (),
        }

        Ok(())
    }
}

/// Policy for shutting down the node upon child actor failure
enum ShutdownPolicy {
    /// Shutdown the node with the given reason
    Shutdown(String),

    /// Continue operations despite the failure
    Continue,
}
