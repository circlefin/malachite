use malachite_actors::node::Msg;
use malachite_actors::prelude::*;
use malachite_actors::util::make_node_actor;
use malachite_test::utils::make_validators;
use malachite_test::{Height, ValidatorSet};

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

    let (tx_decision, mut rx_decision) = tokio::sync::mpsc::channel(32);
    let node = make_node_actor(vs, sk, val.address, tx_decision).await;

    info!("[{index}] Starting...");
    let (actor, join_handle) = Actor::spawn(Some(format!("node-{index}")), node, ()).await?;

    actor.cast(Msg::StartHeight(Height::new(1)))?;

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!("[{index}] Decision at height {height} and round {round}: {value:?}",);
    }

    join_handle.await?;
    Ok(())
}
