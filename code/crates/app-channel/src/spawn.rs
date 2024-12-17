//! Utility functions for spawning the actor system and connecting it to the application.

use eyre::Result;
use tokio::sync::mpsc;

use malachite_app::types::metrics::SharedRegistry;
use malachite_app::types::Keypair;
use malachite_config::Config as NodeConfig;
use malachite_engine::consensus::ConsensusCodec;
use malachite_engine::host::HostRef;
use malachite_engine::network::NetworkRef;
use malachite_engine::sync::SyncCodec;

use crate::app::types::core::Context;
use crate::app::types::metrics::Metrics;
use crate::connector::Connector;
use crate::{AppMsg, NetworkMsg};

pub async fn spawn_host_actor<Ctx>(
    metrics: Metrics,
) -> Result<(HostRef<Ctx>, mpsc::Receiver<AppMsg<Ctx>>)>
where
    Ctx: Context,
{
    let (tx, rx) = mpsc::channel(1);
    let actor_ref = Connector::spawn(tx, metrics).await?;
    Ok((actor_ref, rx))
}

pub async fn spawn_network_actor<Ctx, Codec>(
    cfg: &NodeConfig,
    keypair: Keypair,
    registry: &SharedRegistry,
    codec: Codec,
) -> Result<(NetworkRef<Ctx>, mpsc::Sender<NetworkMsg<Ctx>>)>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    let (tx, mut rx) = mpsc::channel::<NetworkMsg<Ctx>>(1);

    let actor_ref = malachite_app::spawn_network_actor(cfg, keypair, registry, codec).await?;

    tokio::spawn({
        let actor_ref = actor_ref.clone();
        async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = actor_ref.cast(msg.into()) {
                    tracing::error!("Failed to send message to network actor: {e}");
                }
            }
        }
    });

    Ok((actor_ref, tx))
}
