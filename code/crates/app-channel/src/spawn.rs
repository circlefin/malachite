//! Utility functions for spawning the actor system and connecting it to the application.

use malachite_actors::host::HostRef;
use tokio::sync::mpsc;

use crate::app::types::core::Context;
use crate::app::types::metrics::Metrics;
use crate::channel::AppMsg;
use crate::connector::Connector;

pub async fn spawn_host_actor<Ctx>(metrics: Metrics) -> (HostRef<Ctx>, mpsc::Receiver<AppMsg<Ctx>>)
where
    Ctx: Context,
{
    let (tx, rx) = mpsc::channel(1);

    let actor_ref = Connector::spawn(tx, metrics).await.unwrap();

    (actor_ref, rx)
}
