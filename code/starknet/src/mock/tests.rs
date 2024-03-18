use std::str::FromStr;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use crate::mock::types::*;
use crate::mock::MockHost;
use crate::Host;

#[tokio::test]
async fn test_build_new_proposal_normal() -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_millis(500);
    let height = Height::new(1);
    let host = MockHost;

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
async fn test_build_new_proposal_deadline() -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now();
    let height = Height::new(1);
    let host = MockHost;

    let (mut rx_content, rx_hash) = host.build_new_proposal(deadline, height).await;

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert!(
        rx_content.recv().await.is_none(),
        "The content channel should be closed"
    );

    assert!(rx_hash.await.is_err(), "The hash channel should be closed");

    Ok(())
}

#[tokio::test]
async fn test_receive_proposal_normal() -> Result<(), Box<dyn std::error::Error>> {
    let (tx_content, rx_content) = mpsc::channel(10);
    let height = Height::new(1);
    let mock_host = MockHost;

    let rx_hash = mock_host.receive_proposal(rx_content, height).await;

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
async fn test_send_known_proposal_normal() -> Result<(), Box<dyn std::error::Error>> {
    let mock_host = MockHost;
    let block_hash =
        BlockHash::from_str("f8348e0b1df00833cbbbd08f07abdecc10c0efb78829d7828c62a7f36d0cc549")?;

    let (tx_content, handle) = mock_host.send_known_proposal(block_hash).await;

    for i in 0..8 {
        tx_content
            .send(ProposalContent::Tx(TxContent { data: vec![i] }))
            .await?;
    }

    tx_content
        .send(ProposalContent::Proof(ProofContent { data: vec![8] }))
        .await?;

    drop(tx_content); // Trigger the hash comparison

    handle.await?; // No panic or error means success

    Ok(())
}
