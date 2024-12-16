//! Utility functions for spawning the actor system and connecting it to the application.

use eyre::Result;
use tokio::sync::mpsc;

use crate::app::types::core::Context;
use crate::app::types::metrics::Metrics;
use crate::connector::Connector;
use crate::{AppMsg, ConsensusGossipMsg};

use malachite_app::types::{metrics::SharedRegistry, Keypair};
use malachite_config::Config as NodeConfig;
use malachite_engine::consensus::ConsensusCodec;
use malachite_engine::host::HostRef;
use malachite_engine::network::{NetworkMsg, NetworkRef};
use malachite_engine::sync::SyncCodec;

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
) -> Result<(NetworkRef<Ctx>, mpsc::Sender<ConsensusGossipMsg<Ctx>>)>
where
    Ctx: Context,
    Codec: ConsensusCodec<Ctx>,
    Codec: SyncCodec<Ctx>,
{
    let (tx, mut rx) = mpsc::channel(1);

    let actor_ref = malachite_app::spawn_network_actor(cfg, keypair, registry, codec).await?;
    let actor_ref_return = actor_ref.clone();

    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                ConsensusGossipMsg::PublishProposalPart(ppp) => actor_ref
                    .cast(NetworkMsg::PublishProposalPart(ppp))
                    .unwrap(),
            }
        }
    });

    Ok((actor_ref_return, tx))
}
