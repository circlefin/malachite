#![allow(unused_variables)]

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::time::Duration;

use async_trait::async_trait;
use bytesize::ByteSize;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{debug, error};

use malachite_actors::mempool::{MempoolMsg, MempoolRef};
use malachite_common::Round;
use malachite_test::Value;

use crate::mock::types::*;
use crate::Host;

#[derive(Copy, Clone, Debug)]
pub struct MockParams {
    pub max_block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
}

pub struct MockHost {
    params: MockParams,
    mempool: MempoolRef,
}

impl MockHost {
    pub fn new(params: MockParams, mempool: MempoolRef) -> Self {
        Self { params, mempool }
    }
}

#[async_trait]
impl Host for MockHost {
    type Height = Height;
    type BlockHash = BlockHash;
    type MessageHash = MessageHash;
    type ProposalPart = ProposalPart;
    type Signature = Signature;
    type PublicKey = PublicKey;
    type Precommit = Precommit;
    type Validator = Validator;

    #[tracing::instrument(skip(self, deadline))]
    async fn build_new_proposal(
        &self,
        height: Self::Height,
        round: Round,
        deadline: Instant,
    ) -> (
        mpsc::Receiver<Self::ProposalPart>,
        oneshot::Receiver<Self::BlockHash>,
    ) {
        let start = Instant::now();
        let interval = deadline - start;

        let build_duration = interval.mul_f32(self.params.time_allowance_factor);
        let build_deadline = start + build_duration;

        let (tx_part, rx_content) = mpsc::channel(self.params.txs_per_part);
        let (tx_block_hash, rx_block_hash) = oneshot::channel();

        let (params, mempool) = (self.params, self.mempool.clone());
        tokio::spawn(async move {
            let mut tx_batch = Vec::new();
            let mut sequence = 1;
            let mut block_size = 0;
            let mut block_hasher = std::hash::DefaultHasher::new();

            loop {
                debug!(%height, %round, %sequence, "Building local value");

                let txes = mempool
                    .call(
                        |reply| MempoolMsg::TxStream {
                            height: height.as_u64(),
                            num_txes: params.txs_per_part,
                            reply,
                        },
                        Some(build_duration),
                    )
                    .await
                    .unwrap()
                    .unwrap(); // FIXME: Unwrap

                debug!("Reaped {} tx-es from the mempool", txes.len());

                if txes.is_empty() {
                    break;
                }

                let mut tx_count = 0;

                'inner: for tx in txes {
                    if block_size + tx.size_bytes() > params.max_block_size.as_u64() as usize {
                        break 'inner;
                    }

                    block_size += tx.size_bytes();
                    tx.hash(&mut block_hasher);
                    tx_batch.push(tx);
                    tx_count += 1;
                }

                // Simulate execution of reaped txes
                let exec_time = params.exec_time_per_tx * tx_count;
                debug!("Simulating tx execution for {tx_count} tx-es, sleeping for {exec_time:?}");
                tokio::time::sleep(exec_time).await;

                let now = Instant::now();

                if now > build_deadline {
                    error!(
                        "Failed to complete in given interval ({build_duration:?}), took {:?}",
                        now - start,
                    );

                    break;
                }

                sequence += 1;

                debug!(
                    "Created a tx batch with {} tx-es of size {} in {:?}",
                    tx_batch.len(),
                    ByteSize::b(block_size as u64),
                    now - start,
                );

                let part =
                    ProposalPart::TxBatch(TransactionBatch::new(std::mem::take(&mut tx_batch)));

                tx_part.send(part).await.unwrap(); // FIXME: Unwrap

                if now > deadline {
                    let value = Value::new(block_hasher.finish());
                    let proof = vec![42]; // TODO: Compute proof dependent on value
                    let part = ProposalPart::Proof(proof);

                    tx_part.send(part).await.unwrap(); // FIXME: Unwrap

                    break;
                }
            }
        });

        (rx_content, rx_block_hash)
    }

    /// Receive a proposal from a peer.
    ///
    /// Context must support receiving multiple valid proposals on the same (height, round). This
    /// can happen due to a malicious validator, and any one of them can be decided.
    ///
    /// Params:
    /// - height  - The height of the block being proposed.
    /// - content - A channel for receiving the content of the proposal.
    ///             Each element is basically opaque from the perspective of Consensus.
    ///             Examples of what could be sent includes: transaction batch, proof.
    ///             Closing the channel indicates that the proposal is complete.
    ///
    /// Return
    /// - block_hash - ID of the content in the block.
    async fn receive_proposal(
        &self,
        content: mpsc::Receiver<Self::ProposalPart>,
        height: Self::Height,
    ) -> oneshot::Receiver<Self::BlockHash> {
        todo!()
    }

    /// Send a proposal whose content is already known. LOC 16
    ///
    /// Params:
    /// - block_hash - Identifies the content to send.
    ///
    /// Returns:
    /// - content - A channel for sending the content of the proposal.
    async fn send_known_proposal(
        &self,
        block_hash: Self::BlockHash,
    ) -> mpsc::Sender<Self::ProposalPart> {
        todo!()
    }

    /// The set of validators for a given block height. What do we need?
    /// - address      - tells the networking layer where to send messages.
    /// - public_key   - used for signature verification and identification.
    /// - voting_power - used for quorum calculations.
    async fn validators(&self, height: Self::Height) -> Option<BTreeSet<Self::Validator>> {
        todo!()
    }

    // NOTE: Signing of message are left to the `Context` for now
    // /// Fills in the signature field of Message.
    // async fn sign(&self, message: Self::Message) -> Self::SignedMessage;

    /// Validates the signature field of a message. If None returns false.
    async fn validate_signature(
        &self,
        hash: &Self::MessageHash,
        signature: &Self::Signature,
        public_key: &Self::PublicKey,
    ) -> bool {
        todo!()
    }

    /// Update the Context about which decision has been made. It is responsible for pinging any
    /// relevant components in the node to update their states accordingly.
    ///
    /// Params:
    /// - brock_hash - The ID of the content which has been decided.
    /// - precommits - The list of precommits from the round the decision was made (both for and against).
    /// - height     - The height of the decision.
    async fn decision(
        &self,
        block_hash: Self::BlockHash,
        precommits: Vec<Self::Precommit>,
        height: Self::Height,
    ) {
        todo!()
    }
}
