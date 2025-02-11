//! Utility functions for spawning the actor system and connecting it to the application.

use eyre::Result;
use tokio::sync::mpsc;

use malachitebft_app as app;
use malachitebft_app::types::metrics::SharedRegistry;
use malachitebft_app::types::Keypair;
use malachitebft_config::Config as NodeConfig;
use malachitebft_engine::consensus::ConsensusCodec;
use malachitebft_engine::host::HostRef;
use malachitebft_engine::network::NetworkRef;
use malachitebft_engine::sync::SyncCodec;

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
    let (tx, rx) = mpsc::channel(128);
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

    let actor_ref = app::spawn::spawn_network_actor(cfg, keypair, registry, codec).await?;

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
