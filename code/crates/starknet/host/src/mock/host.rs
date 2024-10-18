#![allow(unused_variables)]

use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;
use bytesize::ByteSize;
use malachite_config::VoteExtensionsConfig;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::Instrument;

use malachite_common::{Round, SignedVote};

use crate::mempool::MempoolRef;
use crate::mock::context::MockContext;
use crate::types::*;
use crate::Host;

mod build_proposal;
use build_proposal::build_proposal_task;

#[derive(Copy, Clone, Debug)]
pub struct MockParams {
    pub max_block_size: ByteSize,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
    pub vote_extensions: VoteExtensionsConfig,
}

pub struct MockHost {
    params: MockParams,
    mempool: MempoolRef,
    address: Address,
    validator_set: ValidatorSet,
}

impl MockHost {
    pub fn new(
        params: MockParams,
        mempool: MempoolRef,
        address: Address,
        validator_set: ValidatorSet,
    ) -> Self {
        Self {
            params,
            mempool,
            address,
            validator_set,
        }
    }

    pub fn params(&self) -> MockParams {
        self.params
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
    type Precommit = SignedVote<MockContext>;
    type Validator = Validator;

    #[tracing::instrument(skip_all, fields(%height, %round))]
    async fn build_new_proposal(
        &self,
        height: Self::Height,
        round: Round,
        deadline: Instant,
    ) -> (
        mpsc::Receiver<Self::ProposalPart>,
        oneshot::Receiver<Self::BlockHash>,
    ) {
        let (tx_part, rx_content) = mpsc::channel(self.params.txs_per_part);
        let (tx_block_hash, rx_block_hash) = oneshot::channel();

        tokio::spawn(
            build_proposal_task(
                height,
                round,
                self.address.clone(),
                self.params,
                deadline,
                self.mempool.clone(),
                tx_part,
                tx_block_hash,
            )
            .instrument(tracing::Span::current()),
        );

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
    #[tracing::instrument(skip_all, fields(%height))]
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
    #[tracing::instrument(skip_all, fields(%block_hash))]
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
        Some(self.validator_set.validators.iter().cloned().collect())
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
    #[tracing::instrument(skip_all, fields(height = %_height, block_hash = %_block_hash))]
    async fn decision(
        &self,
        _block_hash: Self::BlockHash,
        _precommits: Vec<Self::Precommit>,
        _height: Self::Height,
    ) {
    }
}
