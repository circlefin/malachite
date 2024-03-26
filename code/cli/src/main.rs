use std::time::Duration;

use malachite_actors::node::Msg;
use malachite_actors::util::make_node_actor;
use malachite_test::utils::make_validators;
use malachite_test::ValidatorSet;

use tracing::info;

const VOTING_POWERS: [u64; 3] = [5, 20, 10];

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let index: usize = std::env::args()
        .nth(1)
        .expect("Error: missing index")
        .parse()
        .expect("Error: invalid index");

    let vs = make_validators(VOTING_POWERS);

    let (val, sk) = vs[index].clone();
    let (vs, _): (Vec<_>, Vec<_>) = vs.into_iter().unzip();
    let vs = ValidatorSet::new(vs);

    info!("[{index}] Starting...");

    let (tx_decision, mut rx_decision) = tokio::sync::mpsc::channel(32);
    let (actor, handle) = make_node_actor(vs, sk, val.address, tx_decision).await;

    tokio::spawn({
        let actor = actor.clone();
        async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("[{index}] Shutting down...");
            actor.stop(None);
        }
    });

    tokio::time::sleep(Duration::from_secs(1)).await;

    actor.cast(Msg::Start)?;

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!("[{index}] Decision at height {height} and round {round}: {value:?}",);
    }

    handle.await?;

    Ok(())
}
