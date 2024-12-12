#![allow(clippy::too_many_arguments)]

use std::sync::Arc;

use bytes::Bytes;
use bytesize::ByteSize;
use eyre::eyre;
use rand::rngs::StdRng;
use rand::{RngCore, SeedableRng};
use sha3::Digest;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{error, trace};

use malachite_core_types::Round;

use crate::host::starknet::StarknetParams;
use crate::mempool::{MempoolMsg, MempoolRef};
use crate::types::*;

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
    private_key: PrivateKey,
    params: StarknetParams,
    deadline: Instant,
    mempool: MempoolRef,
    tx_part: mpsc::Sender<ProposalPart>,
    tx_block_hash: oneshot::Sender<BlockHash>,
) -> Result<(), Box<dyn core::error::Error>> {
    let start = Instant::now();
    let build_duration = (deadline - start).mul_f32(params.time_allowance_factor);

    let mut sequence = 0;
    let mut block_size = 0;
    let mut block_tx_count = 0;
    let mut block_hasher = sha3::Keccak256::new();
    let mut max_block_size_reached = false;

    // Init
    let init = {
        let init = ProposalInit {
            height,
            proposal_round: round,
            proposer: proposer.clone(),
            valid_round: Round::Nil,
        };

        tx_part.send(ProposalPart::Init(init.clone())).await?;
        sequence += 1;

        init
    };

    loop {
        trace!(%height, %round, %sequence, "Building local value");

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

        trace!("Reaped {} transactions from the mempool", reaped_txes.len());

        if reaped_txes.is_empty() {
            break;
        }

        let max_block_size = params.max_block_size.as_u64() as usize;

        let mut txes = Vec::new();
        let mut tx_count = 0;

        for tx in reaped_txes {
            if block_size + tx.size_bytes() > max_block_size {
                max_block_size_reached = true;
                continue;
            }

            block_size += tx.size_bytes();
            tx_count += 1;

            txes.push(tx);
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
            let part = ProposalPart::Transactions(Transactions::new(txes));

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
        let mut rng = StdRng::from_entropy();
        let mut proof = vec![0; 32];
        rng.fill_bytes(&mut proof);

        let part = ProposalPart::BlockProof(BlockProof::new(vec![Bytes::from(proof)]));

        block_hasher.update(part.to_sign_bytes());
        tx_part.send(part).await?;
        sequence += 1;
    }

    let block_hash = BlockHash::new(block_hasher.finalize().into());

    // Fin
    {
        let signature = compute_proposal_signature(&init, &block_hash, &private_key);

        let part = ProposalPart::Fin(ProposalFin { signature });
        tx_part.send(part).await?;
        sequence += 1;
    }

    // Close the channel to signal no more parts to come
    drop(tx_part);

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

pub fn compute_proposal_hash(init: &ProposalInit, block_hash: &BlockHash) -> Hash {
    use sha3::Digest;

    let mut hasher = sha3::Keccak256::new();

    // 1. Block number
    hasher.update(init.height.block_number.to_be_bytes());
    // 2. Fork id
    hasher.update(init.height.fork_id.to_be_bytes());
    // 3. Proposal round
    hasher.update(init.proposal_round.as_i64().to_be_bytes());
    // 4. Valid round
    hasher.update(init.valid_round.as_i64().to_be_bytes());
    // 5. Block hash
    hasher.update(block_hash.as_bytes());

    Hash::new(hasher.finalize().into())
}

pub fn compute_proposal_signature(
    init: &ProposalInit,
    block_hash: &BlockHash,
    private_key: &PrivateKey,
) -> Signature {
    let hash = compute_proposal_hash(init, block_hash);
    private_key.sign(&hash.as_felt())
}
