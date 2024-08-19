#![allow(clippy::too_many_arguments)]

use bytesize::ByteSize;
use eyre::eyre;
use sha3::Digest;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{error, trace};

use malachite_common::Round;

use crate::mempool::{MempoolMsg, MempoolRef};
use crate::mock::host::MockParams;
use crate::types::*;

pub async fn build_proposal_task(
    height: Height,
    round: Round,
    proposer: Address,
    params: MockParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_block_hash: oneshot::Sender<BlockHash>,
) {
    if let Err(e) = run_build_proposal_task(
        height,
        round,
        proposer,
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
    let mut block_hasher = sha3::Keccak256::new();

    // Init
    {
        let part = ProposalPart::init(
            height,
            round,
            sequence,
            proposer.clone(),
            ProposalInit {
                block_number: height.as_u64(),
                fork_id: 1, // TODO: Add fork id
                proposal_round: round,
            },
        );

        block_hasher.update(part.to_sign_bytes());
        tx_part.send(part).await?;
        sequence += 1;
    }

    loop {
        trace!(%height, %round, %sequence, "Building local value");

        let txes = mempool
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

        trace!("Reaped {} transactions from the mempool", txes.len());

        if txes.is_empty() {
            break;
        }

        let mut tx_count = 0;

        'inner: for tx in &txes {
            if block_size + tx.size_bytes() > params.max_block_size.as_u64() as usize {
                max_block_size_reached = true;
                break 'inner;
            }

            block_size += tx.size_bytes();
            tx_count += 1;
        }

        block_tx_count += tx_count;

        let exec_time = params.exec_time_per_tx * tx_count as u32;
        tokio::time::sleep(exec_time).await;

        trace!(
            %sequence,
            "Created a tx batch with {tx_count} tx-es of size {} in {:?}",
            ByteSize::b(block_size as u64),
            start.elapsed()
        );

        // Transactions
        {
            let txes = txes.into_iter().take(tx_count).collect::<Vec<_>>();

            let part = ProposalPart::transactions(
                height,
                round,
                sequence,
                proposer.clone(),
                Transactions::new(txes),
            );

            block_hasher.update(part.to_sign_bytes());
            tx_part.send(part).await?;
            sequence += 1;
        }

        if max_block_size_reached {
            trace!("Max block size reached, stopping tx generation");
            break;
        } else if start.elapsed() > build_duration {
            trace!("Time allowance exceeded, stopping tx generation");
            break;
        }
    }

    // BlockProof
    {
        // TODO: Compute actual "proof"
        let proof = vec![42];
        let part = ProposalPart::block_proof(
            height,
            round,
            sequence,
            proposer.clone(),
            BlockProof::new(vec![proof]),
        );

        block_hasher.update(part.to_sign_bytes());
        tx_part.send(part).await?;
        sequence += 1;
    }

    // Fin
    {
        let part = ProposalPart::fin(
            height,
            round,
            sequence,
            proposer.clone(),
            ProposalFin {
                valid_round: None, // TODO: What's this?
            },
        );

        block_hasher.update(part.to_sign_bytes());
        tx_part.send(part).await?;
        sequence += 1;
    }

    // Close the channel to signal no more parts to come
    drop(tx_part);

    let block_hash = BlockHash::new(block_hasher.finalize().into());
    let block_size = ByteSize::b(block_size as u64);

    trace!(
        tx_count = %block_tx_count, size = %block_size, hash = %block_hash, parts = %sequence,
        "Built block in {:?}", start.elapsed()
    );

    tx_block_hash
        .send(block_hash)
        .map_err(|_| "Failed to send block hash")?;

    Ok(())
}
