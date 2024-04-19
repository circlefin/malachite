use std::str::FromStr;
use std::time::Duration;

use malachite_common::NilOrVal;
use malachite_common::Round;
use malachite_common::VoteType;
use rand::SeedableRng;
use rand_chacha::ChaChaRng;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::time::Instant;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::mock::types::*;
use crate::mock::MockHost;
use crate::Host;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn test_build_new_proposal_normal() -> TestResult {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let deadline = Instant::now() + Duration::from_millis(500);
    let height = Height::new(1);

    let (rx_content, rx_hash) = host.build_new_proposal(deadline, height).await;

    let contents: Vec<_> = ReceiverStream::new(rx_content).collect().await;

    assert!(
        contents.len() >= 8,
        "Expected at least 8 messages, got: {}",
        contents.len()
    );

    assert!(
        matches!(contents.last().unwrap(), ProposalContent::Proof(_)),
        "Expected last message to be ProofContent"
    );

    let hash = rx_hash.await.expect("Expected a hash");
    println!("Hash: {hash}");

    Ok(())
}

#[tokio::test]
async fn test_build_new_proposal_immediate_deadline() -> TestResult {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let deadline = Instant::now();
    let height = Height::new(1);

    let (mut rx_content, rx_hash) = host.build_new_proposal(deadline, height).await;

    sleep(Duration::from_millis(50)).await;

    assert!(
        rx_content.recv().await.is_none(),
        "The content channel should be closed"
    );

    assert!(rx_hash.await.is_err(), "The hash channel should be closed");

    Ok(())
}

#[tokio::test]
async fn test_receive_proposal_normal() -> TestResult {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let height = Height::new(1);
    let (tx_content, rx_content) = mpsc::channel(10);

    let rx_hash = host.receive_proposal(rx_content, height).await;

    for i in 0..8 {
        tx_content
            .send(ProposalContent::Tx(TxContent { data: vec![i] }))
            .await?;
    }

    tx_content
        .send(ProposalContent::Proof(ProofContent { data: vec![8] }))
        .await?;

    drop(tx_content);

    let hash = rx_hash.await.expect("Expected a hash");
    println!("Hash: {hash}");

    Ok(())
}

#[tokio::test]
async fn test_receive_proposal_no_content() -> TestResult {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let height = Height::new(1);
    let (tx_content, rx_content) = mpsc::channel(10);

    let rx_hash = host.receive_proposal(rx_content, height).await;

    drop(tx_content); // Close the channel without sending any content

    // Verify that a hash is still produced, likely representing an empty content set
    let hash = rx_hash.await.expect("Expected a hash despite no content");
    println!("{hash}");

    Ok(())
}

#[tokio::test]
async fn test_send_known_proposal_correct_hash() -> TestResult {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let block_hash =
        BlockHash::from_str("f8348e0b1df00833cbbbd08f07abdecc10c0efb78829d7828c62a7f36d0cc549")?;

    let tx_content = host.send_known_proposal(block_hash).await;

    for i in 0..8 {
        tx_content
            .send(ProposalContent::Tx(TxContent { data: vec![i] }))
            .await?;
    }

    tx_content
        .send(ProposalContent::Proof(ProofContent { data: vec![8] }))
        .await?;

    drop(tx_content); // Trigger the hash comparison

    sleep(Duration::from_millis(100)).await;

    assert_eq!(host.last_error(), None);

    Ok(())
}

#[tokio::test]
async fn test_send_known_proposal_incorrect_hash() {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let block_hash = BlockHash::new([255; 32]); // Example hash unlikely to match actual content

    let tx_content = host.send_known_proposal(block_hash).await;

    for i in 0..8 {
        tx_content
            .send(ProposalContent::Tx(TxContent { data: vec![i] }))
            .await
            .unwrap();
    }

    tx_content
        .send(ProposalContent::Proof(ProofContent { data: vec![8] }))
        .await
        .unwrap();

    drop(tx_content); // Trigger the hash comparison

    sleep(Duration::from_millis(100)).await; // Wait for the task to complete

    assert!(host.last_error().unwrap().contains("Invalid hash"));
}

#[tokio::test]
async fn test_sign_message_proposal() {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let message = Message::Proposal(Proposal {
        height: Height::new(1),
        round: Round::new(0),
        value: ProposalContent::Tx(TxContent { data: vec![42] }),
        pol_round: Round::Nil,
        validator_address: Address::new([1; 20]),
    });

    let message_hash = message.hash();
    let signed_message = host.sign(message).await;

    let valid = host
        .validate_signature(&message_hash, &signed_message.signature, &host.public_key())
        .await;

    assert!(valid, "Expected the signature to be valid");
}

#[tokio::test]
async fn test_sign_message_vote() {
    let mut rng = ChaChaRng::from_seed([42; 32]);
    let host = MockHost::new(&mut rng);

    let message = Message::Vote(Vote {
        typ: VoteType::Precommit,
        height: Height::new(1),
        round: Round::new(0),
        value: NilOrVal::Val(BlockHash::new([42; 32])),
        validator_address: Address::new([1; 20]),
    });

    let message_hash = message.hash();
    let signed_message = host.sign(message).await;

    let valid = host
        .validate_signature(&message_hash, &signed_message.signature, &host.public_key())
        .await;

    assert!(valid, "Expected the signature to be valid");
}
