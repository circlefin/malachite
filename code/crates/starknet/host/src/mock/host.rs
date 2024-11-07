use std::collections::BTreeSet;
use std::time::Duration;

use async_trait::async_trait;
use bytesize::ByteSize;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::Instrument;

use malachite_common::{CommitCertificate, Round, SignedVote};
use malachite_config::VoteExtensionsConfig;
use malachite_consensus::ValuePayload;

use crate::mempool::MempoolRef;
use crate::mock::context::MockContext;
use crate::part_store::PartStore;
use crate::types::*;
use crate::Host;

mod build_proposal;
use build_proposal::{build_proposal_task, repropose_task};

#[derive(Copy, Clone, Debug)]
pub struct MockParams {
    pub max_block_size: ByteSize,
    pub value_payload: ValuePayload,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
    pub max_retain_blocks: usize,
    pub vote_extensions: VoteExtensionsConfig,
}

pub struct MockHost {
    pub params: MockParams,
    pub mempool: MempoolRef,
    pub address: Address,
    pub private_key: PrivateKey,
    pub validator_set: ValidatorSet,
    pub part_store: PartStore<MockContext>,
}

impl MockHost {
    pub fn new(
        params: MockParams,
        mempool: MempoolRef,
        address: Address,
        private_key: PrivateKey,
        validator_set: ValidatorSet,
    ) -> Self {
        Self {
            params,
            mempool,
            address,
            private_key,
            validator_set,
            part_store: Default::default(),
        }
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
                self.private_key,
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
    #[tracing::instrument(skip_all, fields(height = %_height))]
    async fn receive_proposal(
        &self,
        _content: mpsc::Receiver<Self::ProposalPart>,
        _height: Self::Height,
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
    ) -> mpsc::Receiver<Self::ProposalPart> {
        let parts = self.part_store.all_parts_by_value_id(&block_hash);
        let (tx_part, rx_content) = mpsc::channel(self.params.txs_per_part);

        tokio::spawn(
            repropose_task(block_hash, tx_part, parts).instrument(tracing::Span::current()),
        );

        rx_content
    }

    /// The set of validators for a given block height. What do we need?
    /// - address      - tells the networking layer where to send messages.
    /// - public_key   - used for signature verification and identification.
    /// - voting_power - used for quorum calculations.
    async fn validators(&self, _height: Self::Height) -> Option<BTreeSet<Self::Validator>> {
        Some(self.validator_set.validators.iter().cloned().collect())
    }

    /// Sign a message hash
    async fn sign(&self, message: Self::MessageHash) -> Self::Signature {
        self.private_key.sign(&message.as_felt())
    }

    /// Validates the signature field of a message. If None returns false.
    async fn validate_signature(
        &self,
        hash: &Self::MessageHash,
        signature: &Self::Signature,
        public_key: &Self::PublicKey,
    ) -> bool {
        public_key.verify(&hash.as_felt(), signature)
    }

    /// Update the Context about which decision has been made. It is responsible for pinging any
    /// relevant components in the node to update their states accordingly.
    ///
    /// Params:
    /// - brock_hash - The ID of the content which has been decided.
    /// - precommits - The list of precommits from the round the decision was made (both for and against).
    /// - height     - The height of the decision.
    #[tracing::instrument(skip_all, fields(height = %_certificate.height, block_hash = %_certificate.value_id))]
    async fn decision(&self, _certificate: CommitCertificate<MockContext>) {}
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
