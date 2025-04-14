#![allow(clippy::too_many_arguments)]

use std::sync::Arc;
use std::time::SystemTime;

use bytesize::ByteSize;
use eyre::eyre;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{debug, error, trace};

use malachitebft_core_types::Round;

use crate::host::starknet::StarknetParams;
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::types::*;

const PROTOCOL_VERSION: &str = "0.13.0";

pub async fn build_proposal_task(
    height: Height,
    round: Round,
    proposer: Address,
    private_key: PrivateKey,
    params: StarknetParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_block_hash: oneshot::Sender<BlockHash>,
) {
    if let Err(e) = run_build_proposal_task(
        height,
        round,
        proposer,
        private_key,
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
    proposer: Address,
    _private_key: PrivateKey,
    params: StarknetParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_value_id: oneshot::Sender<Hash>,
) -> Result<(), Box<dyn core::error::Error>> {
    let start = Instant::now();
    let build_duration = (deadline - start).mul_f32(params.time_allowance_factor);
    let build_deadline = start + build_duration;

    let mut sequence = 0;
    let mut block_tx_count = 0;
    let mut block_size = 0;

    trace!(%height, %round, "Building local value");

    // Init
    {
        let part = ProposalPart::Init(ProposalInit {
            height,
            round,
            proposer,
            valid_round: Round::Nil,
        });

        tx_part.send(part).await?;
        sequence += 1;
    }

    let now = SystemTime::UNIX_EPOCH.elapsed().unwrap().as_secs();

    // Block Info
    {
        let part = ProposalPart::BlockInfo(BlockInfo {
            height,
            builder: proposer,
            timestamp: now,
            l1_gas_price_wei: 0,
            l1_data_gas_price_wei: 0,
            l2_gas_price_fri: 0,
            eth_to_strk_rate: 0,
            l1_da_mode: L1DataAvailabilityMode::Blob,
        });

        tx_part.send(part).await?;
        sequence += 1;
    }

    let max_block_size = params.max_block_size.as_u64() as usize;

    'reap: loop {
        let reaped_txes = mempool
            .call(
                |reply| MempoolMsg::Reap {
                    height: height.as_u64(),
                    num_txes: params.txs_per_part,
                    reply,
                },
                Some(build_duration),
            )
            .await?
            .success_or(eyre!("Failed to reap transactions from the mempool"))?;

        debug!("Reaped {} transactions from the mempool", reaped_txes.len());

        if reaped_txes.is_empty() {
            debug!("No more transactions to reap");
            break 'reap;
        }

        let mut txes = Vec::new();
        let mut full_block = false;

        'txes: for tx in reaped_txes {
            if block_size + tx.size_bytes() > max_block_size {
                full_block = true;
                break 'txes;
            }

            block_size += tx.size_bytes();
            block_tx_count += 1;

            txes.push(tx);
        }

        let exec_time = params.exec_time_per_tx * txes.len() as u32;
        tokio::time::sleep(exec_time).await;

        trace!(
            %sequence,
            "Created a tx batch with {} tx-es of size {} in {:?}",
            txes.len(),
            ByteSize::b(block_size as u64),
            start.elapsed()
        );

        // Transactions
        {
            let part = ProposalPart::Transactions(TransactionBatch::new(txes));
            tx_part.send(part).await?;
            sequence += 1;
        }

        if full_block {
            debug!("Max block size reached, stopping tx generation");
            break 'reap;
        } else if start.elapsed() >= build_duration {
            debug!("Time allowance exceeded, stopping tx generation");
            break 'reap;
        }
    }

    if params.stable_block_times {
        // Sleep for the remaining time, in order to not break tests
        // by producing blocks too quickly
        tokio::time::sleep(build_deadline - Instant::now()).await;
    }

    // Proposal Commitment
    {
        let part = ProposalPart::Commitment(Box::new(ProposalCommitment {
            height,
            parent_commitment: Hash::new([0; 32]),
            builder: proposer,
            timestamp: now,
            protocol_version: PROTOCOL_VERSION.to_string(),
            old_state_root: Hash::new([0; 32]),
            state_diff_commitment: Hash::new([0; 32]),
            transaction_commitment: Hash::new([0; 32]),
            event_commitment: Hash::new([0; 32]),
            receipt_commitment: Hash::new([0; 32]),
            concatenated_counts: Felt::ONE,
            l1_gas_price_fri: 0,
            l1_data_gas_price_fri: 0,
            l2_gas_price_fri: 0,
            l2_gas_used: 0,
            l1_da_mode: L1DataAvailabilityMode::Blob,
        }));

        tx_part.send(part).await?;
        sequence += 1;
    }

    // TODO: Compute the actual propoosal commitment hash
    let proposal_commitment_hash = Hash::new([42; 32]);

    // Fin
    {
        let part = ProposalPart::Fin(ProposalFin {
            proposal_commitment_hash,
        });
        tx_part.send(part).await?;
        sequence += 1;
    }

    // Close the channel to signal no more parts to come
    drop(tx_part);

    let block_size = ByteSize::b(block_size as u64);

    debug!(
        tx_count = %block_tx_count, size = %block_size, %proposal_commitment_hash, parts = %sequence,
        "Built block in {:?}", start.elapsed()
    );

    tx_value_id
        .send(proposal_commitment_hash)
        .map_err(|_| "Failed to send proposal commitment hash")?;

    Ok(())
}

pub async fn repropose_task(
    block_hash: Hash,
    tx_part: mpsc::Sender<ProposalPart>,
    parts: Vec<Arc<ProposalPart>>,
) {
    if let Err(e) = run_repropose_task(block_hash, tx_part, parts).await {
        error!("Failed to restream proposal: {e:?}");
    }
}

async fn run_repropose_task(
    _block_hash: Hash,
    tx_part: mpsc::Sender<ProposalPart>,
    parts: Vec<Arc<ProposalPart>>,
) -> Result<(), Box<dyn core::error::Error>> {
    for part in parts {
        let part = Arc::unwrap_or_clone(part);
        tx_part.send(part).await?;
    }
    Ok(())
}
