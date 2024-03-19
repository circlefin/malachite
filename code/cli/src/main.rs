use malachite_node::util::make_gossip_node;
use malachite_test::utils::make_validators;
use malachite_test::ValidatorSet;

use tracing::info;

const VOTING_POWERS: [u64; 3] = [5, 20, 10];

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
    tracing_subscriber::fmt::init();

    let index: usize = std::env::args()
        .nth(1)
        .expect("Error: missing index")
        .parse()
        .expect("Error: invalid index");

    // Validators keys are deterministic and match the ones in the config file
    let vs = make_validators(VOTING_POWERS);

    let (val, sk) = vs[index].clone();
    let (vs, _): (Vec<_>, Vec<_>) = vs.into_iter().unzip();
    let vs = ValidatorSet::new(vs);

    let node = make_gossip_node(vs, sk, val.address).await;

    info!("[{index}] Starting...");

    let mut handle = node.run().await;

    loop {
        if let Some((height, round, value)) = handle.wait_decision().await {
            info!("[{index}] Decision at height {height} and round {round}: {value:?}",);
        }
    }
}
