use clap::Parser;
use logging::DebugSection;
use malachite_actors::util::make_node_actor;
use malachite_test::utils::make_validators;
use malachite_test::ValidatorSet;

use tracing::info;

use crate::logging::LogLevel;

#[derive(clap::Parser)]
pub struct Args {
    #[clap(
        short,
        long,
        help = "Index of this node in the validator set (0, 1, or 2)"
    )]
    pub index: usize,

    #[clap(
        short,
        long = "debug",
        help = "Enable debug output for the given comma-separated sections",
        value_enum,
        value_delimiter = ','
    )]
    debug: Vec<DebugSection>,
}

const VOTING_POWERS: [u64; 3] = [11, 10, 10];

mod logging;

#[tokio::main(flavor = "current_thread")]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let index = args.index;

    logging::init(LogLevel::Debug, &args.debug);

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

    while let Some((height, round, value)) = rx_decision.recv().await {
        info!("[{index}] Decision at height {height} and round {round}: {value:?}",);
    }

    handle.await?;

    Ok(())
}
