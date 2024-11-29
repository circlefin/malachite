#[tokio::test]
async fn test_app() {
    use crate::channel::{AppMsg, ConsensusMsg};
    use crate::run::run;
    use libp2p_identity::Keypair;
    use malachite_actors::host::LocallyProposedValue;
    use malachite_common::{Round, Validity};
    use malachite_config::Config;
    use malachite_consensus::ProposedValue;
    use malachite_test::{
        Address, Height, PrivateKey, TestCodec, TestContext, Validator, ValidatorSet, Value,
    };
    use rand::rngs::OsRng;

    let cfg = Config::default();
    let private_key = PrivateKey::generate(OsRng);
    let public_key = private_key.public_key();
    let address = Address::from_public_key(&public_key);
    let ctx = TestContext::new(private_key.clone());
    let codec = TestCodec;

    let keypair = Keypair::ed25519_from_bytes(private_key.inner().to_bytes()).unwrap();

    let validator = Validator::new(public_key, 10);
    let validator_set = ValidatorSet::new(vec![validator]);

    let mut consensus_rx = run(
        cfg,
        None,
        ctx,
        codec,
        keypair,
        address,
        validator_set.clone(),
    )
    .await
    .unwrap();

    const INITIAL_HEIGHT: u64 = 1;
    let mut node_height = 1;
    let mut node_round = 0;

    println!("Starting loop...");
    loop {
        println!(
            "Iteration of loop at height {}, round {}",
            node_height, node_round
        );
        match consensus_rx.recv().await {
            Some(msg) => match msg {
                AppMsg::ConsensusReady { reply_to } => {
                    println!("ConsensusReady");
                    if let Err(_) =
                        reply_to.send(ConsensusMsg::StartHeight(Height::new(node_height)))
                    {
                        println!("Failed to send ConsensusReady reply");
                    }
                }
                AppMsg::StartedRound {
                    height,
                    round,
                    proposer,
                } => {
                    println!(
                        "StartedRound: height={}, round={}, proposer={}",
                        height, round, proposer
                    );
                }
                AppMsg::GetValue {
                    height,
                    round,
                    timeout_duration,
                    address,
                    reply_to,
                } => {
                    println!(
                        "GetValue: height={}, round={}, timeout_duration={:?}, address={}",
                        height, round, timeout_duration, address
                    );
                    if let Err(_) = reply_to.send(LocallyProposedValue::new(
                        height,
                        round,
                        Value::new(height.as_u64()),
                        None,
                    )) {
                        println!("Failed to send GetValue reply");
                    }
                }
                AppMsg::RestreamValue {
                    height,
                    round,
                    valid_round,
                    address,
                    value_id,
                } => {
                    println!("RestreamValue: height={}, round={}, valid_round={}, address={}, value_id={}", height, round, valid_round, address, value_id);
                }
                AppMsg::GetEarliestBlockHeight { reply_to } => {
                    println!("GetEarliestBlockHeight");
                    if let Err(_) = reply_to.send(Height::new(INITIAL_HEIGHT)) {
                        println!("Failed to send GetEarliestBlockHeight reply");
                    }
                }
                AppMsg::ReceivedProposalPart {
                    from,
                    part,
                    reply_to,
                } => {
                    println!("ReceivedProposalPart: from={}, part={:?}", from, part);
                    if let Err(_) = reply_to.send(ProposedValue {
                        height: Height::new(node_height),
                        round: Round::new(node_round),
                        valid_round: Round::new(node_round),
                        validator_address: address,
                        value: Value::new(node_height),
                        validity: Validity::Valid,
                        extension: None,
                    }) {
                        println!("Failed to send ReceivedProposalPart reply");
                    }
                }
                AppMsg::GetValidatorSet { height, reply_to } => {
                    println!("GetValidatorSet: height={}", height);
                    if let Err(_) = reply_to.send(validator_set.clone()) {
                        println!("Failed to send GetValidatorSet reply");
                    }
                }
                AppMsg::Decided {
                    certificate,
                    reply_to,
                } => {
                    println!("Decided: certificate={:?}", certificate);
                    node_height += 1;
                    node_round = 0;
                    if node_height == 5 {
                        break;
                    }
                    if let Err(_) =
                        reply_to.send(ConsensusMsg::StartHeight(Height::new(node_height)))
                    {
                        println!("Failed to send Decided reply");
                    }
                }
                AppMsg::GetDecidedBlock { height, reply_to } => {
                    println!("GetDecidedBlock: height={}", height);
                    if let Err(_) = reply_to.send(None) {
                        println!("Failed to send GetDecidedBlock reply");
                    }
                }
                AppMsg::ProcessSyncedBlock {
                    height,
                    round,
                    validator_address,
                    block_bytes,
                    reply_to,
                } => {
                    println!("ProcessSyncedBlock: height={}, round={}, validator_address={}, block_bytes.len()={}", height, round, validator_address, block_bytes.len());
                    if let Err(_) = reply_to.send(ProposedValue {
                        height: Height::new(node_height),
                        round: Round::new(node_round),
                        valid_round: Round::new(node_round),
                        validator_address: address,
                        value: Value::new(node_height),
                        validity: Validity::Valid,
                        extension: None,
                    }) {
                        println!("Failed to send ProcessSyncedBlock reply");
                    }
                }
            },
            None => {
                println!("Channel is closed.")
            }
        }
    }
}
