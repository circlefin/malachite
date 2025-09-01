#![allow(clippy::too_many_arguments)]
use sha3::Digest;
use std::time::SystemTime;

use bytesize::ByteSize;
use eyre::eyre;
use tokio::time::Instant;
use tracing::{debug, trace};

use malachitebft_core_types::Round;

use crate::app::AppParams;
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::types::{Address, Block, Hash, Height, TransactionBatch};

pub async fn build_proposal_task(
    height: Height,
    round: Round,
    _proposer: Address, // TODO: add to block def
    params: AppParams,
    deadline: Instant,
    mempool: MempoolRef,
) -> Result<Block, Box<dyn core::error::Error + Send + Sync>> {
    // TODO - if needed, use this deadline to stop the build_new_proposal
    let start = Instant::now();
    let build_duration = (deadline - start).mul_f32(params.time_allowance_factor);

    let _build_deadline = start + build_duration;
    let _now = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();

    let mut block_tx_count = 0;
    let mut block_size = 0;

    trace!(%height, %round, "Building local value");

    let max_block_size = params.max_block_size.as_u64() as usize;
    let mut hasher = sha3::Keccak256::new();

    let mut txes = Vec::new();
    let mut full_block = false;

    'reap: loop {
        let reaped_txes = mempool
            .call(
                |reply| MempoolMsg::Reap {
                    height: height.as_u64(),
                    num_txes: 1,
                    reply,
                },
                Some(build_duration),
            )
            .await?
            .success_or(eyre!("Failed to reap transactions from the mempool"))?;

        if reaped_txes.is_empty() {
            debug!("No more transactions to reap");
            break 'reap;
        }

        'txes: for tx in reaped_txes {
            if block_size + tx.size_bytes() > max_block_size {
                full_block = true;
                break 'txes;
            }

            block_size += tx.size_bytes();
            block_tx_count += 1;

            txes.push(tx.clone());
            hasher.update(tx.clone().hash().as_bytes());
        }

        let exec_time = params.exec_time_per_tx * txes.len() as u32;
        tokio::time::sleep(exec_time).await;

        if full_block {
            debug!("Max block size reached, stopping tx generation");
            break 'reap;
        } else if start.elapsed() >= build_duration {
            debug!("Time allowance exceeded, stopping tx generation");
            break 'reap;
        }
    }

    let transaction_commitment = Hash::new(hasher.finalize().into());

    let built_block = Block::new(
        height,
        TransactionBatch::new(txes),
        transaction_commitment.clone(),
    );

    let block_size = ByteSize::b(block_size as u64);
    debug!(
        tx_count = %block_tx_count, size = %block_size, transaction_commitment = ?transaction_commitment,
        "Built block in {:?}", start.elapsed()
    );

    Ok(built_block)
}
