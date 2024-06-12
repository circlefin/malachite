#![allow(unused_variables)]

use std::collections::BTreeSet;

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::mock::types::*;
use crate::Host;

pub struct MockHost;

#[async_trait]
impl Host for MockHost {
    type Height = Height;
    type BlockHash = BlockHash;
    type MessageHash = MessageHash;
    type ProposalContent = ProposalContent;
    type Signature = Signature;
    type PublicKey = PublicKey;
    type Precommit = Precommit;
    type Validator = Validator;

    async fn build_new_proposal(
        &self,
        deadline: Instant,
        height: Self::Height,
    ) -> (
        mpsc::Receiver<Self::ProposalContent>,
        oneshot::Receiver<Self::BlockHash>,
    ) {
        todo!()
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
        content: mpsc::Receiver<Self::ProposalContent>,
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
    ) -> mpsc::Sender<Self::ProposalContent> {
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
