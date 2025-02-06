use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use bytesize::ByteSize;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{debug, Instrument};

use malachitebft_config::VoteExtensionsConfig;
use malachitebft_core_consensus::ValuePayload;
use malachitebft_core_types::{CommitCertificate, Round, SignedVote, VoteExtensions};

use crate::host::Host;
use crate::mempool::MempoolRef;
use crate::part_store::PartStore;
use crate::types::*;

use super::proposal::{build_proposal_task, repropose_task};

#[derive(Copy, Clone, Debug)]
pub struct StarknetParams {
    pub max_block_size: ByteSize,
    pub value_payload: ValuePayload,
    pub tx_size: ByteSize,
    pub txs_per_part: usize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
    pub max_retain_blocks: usize,
    pub vote_extensions: VoteExtensionsConfig,
}

pub struct StarknetHost {
    pub params: StarknetParams,
    pub mempool: MempoolRef,
    pub address: Address,
    pub private_key: PrivateKey,
    pub validator_set: ValidatorSet,
    pub part_store: PartStore<MockContext>,
    pub vote_extensions: HashMap<Height, VoteExtensions<MockContext>>,
}

impl StarknetHost {
    pub fn new(
        params: StarknetParams,
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
            vote_extensions: Default::default(),
        }
    }

    pub fn generate_vote_extension(&self, _height: Height, _round: Round) -> Option<Bytes> {
        use rand::RngCore;

        if !self.params.vote_extensions.enabled {
            return None;
        }

        let size = self.params.vote_extensions.size.as_u64() as usize;
        debug!(%size, "Vote extensions are enabled");

        let mut bytes = vec![0u8; size];
        rand::thread_rng().fill_bytes(&mut bytes);

        Some(Bytes::from(bytes))
    }
}

#[async_trait]
impl Host for StarknetHost {
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
        &mut self,
        height: Self::Height,
        round: Round,
        deadline: Instant,
    ) -> (
        mpsc::Receiver<Self::ProposalPart>,
        oneshot::Receiver<Self::BlockHash>,
    ) {
        let (tx_part, rx_content) = mpsc::channel(self.params.txs_per_part);
        let (tx_block_hash, rx_block_hash) = oneshot::channel();

        let vote_extensions = self.vote_extensions.remove(&height).unwrap_or_default();

        tokio::spawn(
            build_proposal_task(
                height,
                round,
                self.address,
                self.private_key,
                vote_extensions,
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
