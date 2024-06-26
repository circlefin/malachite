#![allow(clippy::too_many_arguments)]

use bytesize::ByteSize;
use sha2::{Digest, Sha256};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{error, trace};

use malachite_actors::mempool::{MempoolMsg, MempoolRef};
use malachite_common::Round;

use crate::mock::host::MockParams;
use crate::mock::types::*;

pub async fn build_proposal_task(
    height: Height,
    round: Round,
    params: MockParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_block_hash: oneshot::Sender<BlockHash>,
) {
    if let Err(e) = run_build_proposal_task(
        height,
        round,
        params,
        deadline,
        mempool,
        tx_part,
        tx_block_hash,
    )
    .await
    {
        error!("Failed to build proposal: {e:?}");
    }
}

async fn run_build_proposal_task(
    height: Height,
    round: Round,
    params: MockParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_block_hash: oneshot::Sender<BlockHash>,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let build_duration = (deadline - start).mul_f32(params.time_allowance_factor);

    let mut sequence = 1;
    let mut block_size = 0;
    let mut block_tx_count = 0;
    let mut max_block_size_reached = false;
    let mut block_hasher = Sha256::new();

    loop {
        trace!(%height, %round, %sequence, "Building local value");

        let txes = mempool
            .call(
                |reply| MempoolMsg::TxStream {
                    height: height.as_u64(),
                    num_txes: params.txs_per_part,
                    reply,
                },
                Some(build_duration),
            )
            .await?
            .success_or("Failed to get tx-es from the mempool")?;

        trace!("Reaped {} tx-es from the mempool", txes.len());

        if txes.is_empty() {
            break;
        }

        let mut tx_count = 0;

        'inner: for tx in &txes {
            if block_size + tx.size_bytes() > params.max_block_size.as_u64() as usize {
                max_block_size_reached = true;
                break 'inner;
            }

            block_hasher.update(tx.as_bytes());

            block_size += tx.size_bytes();
            tx_count += 1;
        }

        let txes = txes.into_iter().take(tx_count).collect::<Vec<_>>();

        tokio::time::sleep(params.exec_time_per_tx * tx_count as u32).await;

        block_tx_count += tx_count;

        trace!(
            %sequence,
            "Created a tx batch with {tx_count} tx-es of size {} in {:?}",
            ByteSize::b(block_size as u64),
            start.elapsed()
        );

        let part = ProposalPart::TxBatch(sequence, TransactionBatch::new(txes));
        tx_part.send(part).await?;

        sequence += 1;

        if max_block_size_reached {
            trace!("Max block size reached, stopping tx generation");
            break;
        } else if start.elapsed() > build_duration {
            trace!("Time allowance exceeded, stopping tx generation");
            break;
        }
    }

    // TODO: Compute actual "proof"
    let proof = vec![42];

    let hash = block_hasher.finalize();
    let block_hash = BlockHash::new(hash.into());
    let block_metadata = BlockMetadata::new(proof, block_hash);
    let part = ProposalPart::Metadata(sequence, block_metadata);
    let block_size = ByteSize::b(block_size as u64);

    trace!("Built block with {block_tx_count} tx-es of size {block_size} and hash {block_hash}, in {sequence} block parts");

    // Send and then close the channel
    tx_part.send(part).await?;
    drop(tx_part);

    tx_block_hash
        .send(block_hash)
        .map_err(|_| "Failed to send block hash")?;

    Ok(())
}
