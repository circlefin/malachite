use std::collections::BTreeSet;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use malachitebft_core_types::{CommitCertificate, Round};

use crate::types::MockContext;

pub mod proposal;
pub mod starknet;
pub mod state;

pub use starknet::{StarknetHost, StarknetParams};

#[async_trait]
pub trait Host {
    type Height;
    type BlockHash;
    type ProposalPart;
    type MessageHash;
    type Signature;
    type PublicKey;
    type Precommit;
    type Validator;

    /// Initiate building a proposal.
    ///
    /// Params:
    /// - deadline - When the Context must stop adding new TXs to the block.
    /// - height   - The height of the block being proposed.
    ///
    /// Return
    /// - content    - A channel for sending the content of the proposal.
    ///                Each element is basically opaque from the perspective of Consensus.
    ///                Examples of what could be sent includes: transaction batch, proof.
    ///                Closing the channel indicates that the proposal is complete.
    /// - block_hash - ID of the content in the block.
    async fn build_new_proposal(
        &self,
        height: Self::Height,
        round: Round,
        deadline: Instant,
    ) -> (
        mpsc::Receiver<Self::ProposalPart>,
        oneshot::Receiver<Self::BlockHash>,
    );

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
    ) -> oneshot::Receiver<Self::BlockHash>;

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
    ) -> mpsc::Receiver<Self::ProposalPart>;

    /// The set of validators for a given block height. What do we need?
    /// - address      - tells the networking layer where to send messages.
    /// - public_key   - used for signature verification and identification.
    /// - voting_power - used for quorum calculations.
    async fn validators(&self, height: Self::Height) -> Option<BTreeSet<Self::Validator>>;

    /// Sign the given message hash
    async fn sign(&self, message: Self::MessageHash) -> Self::Signature;

    /// Validates the signature of a message hash.
    async fn validate_signature(
        &self,
        hash: &Self::MessageHash,
        signature: &Self::Signature,
        public_key: &Self::PublicKey,
    ) -> bool;

    /// Update the Context about which decision has been made. It is responsible for pinging any
    /// relevant components in the node to update their states accordingly.
    ///
    /// Params:
    /// - brock_hash - The ID of the content which has been decided.
    /// - precommits - The list of precommits from the round the decision was made (both for and against).
    /// - height     - The height of the decision.
    async fn decision(&self, certificate: CommitCertificate<MockContext>);
}
