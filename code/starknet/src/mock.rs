use malachite_common::signature::SignerMut;
use malachite_test::PrivateKey;
use rand::{CryptoRng, Rng, RngCore, SeedableRng};
use rand_chacha::ChaChaRng;
use sha2::{Digest, Sha256};

mod types;
pub use types::*;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::Host;

#[derive(Clone)]
pub struct MockHost {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    private_key: PrivateKey,
    last_error: Option<String>,
}

impl MockHost {
    pub fn new(rng: impl RngCore + CryptoRng) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                private_key: PrivateKey::generate(rng),
                last_error: None,
            })),
        }
    }

    pub fn last_error(&self) -> Option<String> {
        self.inner.read().unwrap().last_error.clone()
    }

    pub fn set_last_error(&self, error: Option<String>) {
        self.inner.write().unwrap().last_error = error;
    }

    pub fn public_key(&self) -> PublicKey {
        self.inner.read().unwrap().private_key.public_key()
    }
}

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
    type Message = Message;
    type SignedMessage = SignedMessage;

    async fn build_new_proposal(
        &self,
        deadline: Instant,
        height: Self::Height,
    ) -> (
        mpsc::Receiver<Self::ProposalContent>,
        oneshot::Receiver<Self::BlockHash>,
    ) {
        let (tx_content, rx_content) = mpsc::channel(10);
        let (tx_hash, rx_hash) = oneshot::channel();

        let time_left = deadline.duration_since(Instant::now());
        let step = time_left / 10;

        tokio::spawn(async move {
            let mut hasher = Sha256::new();
            hasher.update(height.as_u64().to_le_bytes());

            let mut rng = ChaChaRng::from_seed(seed_from_height(height));

            for _ in 0u64..8 {
                if Instant::now() >= deadline {
                    drop(tx_content);
                    drop(tx_hash);

                    // FIXME: Do we return or we still emit the proof/hash?
                    return;
                }

                tokio::time::sleep(step).await;

                let content = Tx {
                    data: rng.gen::<u64>().to_le_bytes().to_vec(),
                };
                hasher.update(&content.data);

                tx_content.send(ProposalContent::Tx(content)).await.unwrap();
            }

            let proof = Proof {
                data: rng.gen::<u64>().to_le_bytes().to_vec(),
            };
            hasher.update(&proof.data);

            tx_content
                .send(ProposalContent::Proof(proof))
                .await
                .unwrap();

            drop(tx_content);

            let hash = hasher.finalize().into();
            tx_hash.send(BlockHash::new(hash)).unwrap();
        });

        (rx_content, rx_hash)
    }

    async fn receive_proposal(
        &self,
        mut content: mpsc::Receiver<Self::ProposalContent>,
        height: Self::Height,
    ) -> oneshot::Receiver<Self::BlockHash> {
        let (tx_hash, rx_hash) = oneshot::channel();

        tokio::spawn(async move {
            let mut hasher = Sha256::new();
            hasher.update(height.as_u64().to_le_bytes());

            while let Some(proposal) = content.recv().await {
                match proposal {
                    ProposalContent::Tx(tx) => hasher.update(&tx.data),
                    ProposalContent::Proof(proof) => hasher.update(&proof.data),
                }
            }

            let hash = hasher.finalize().into();
            tx_hash.send(BlockHash::new(hash)).unwrap();
        });

        rx_hash
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
        let this = self.clone();

        let (tx_content, mut rx_content) = mpsc::channel(10);

        tokio::spawn(async move {
            let mut hasher = Sha256::new();

            while let Some(content) = rx_content.recv().await {
                match content {
                    ProposalContent::Tx(tx) => hasher.update(&tx.data),
                    ProposalContent::Proof(proof) => hasher.update(&proof.data),
                }
            }

            let hash = BlockHash::new(hasher.finalize().into());

            if hash != block_hash {
                this.set_last_error(Some(format!("Invalid hash: {hash} != {block_hash}")));
            }
        });

        tx_content
    }

    /// The set of validators for a given block height. What do we need?
    /// - address      - tells the networking layer where to send messages.
    /// - public_key   - used for signature verification and identification.
    /// - voting_power - used for quorum calculations.
    async fn validators(&self, _height: Self::Height) -> Option<BTreeSet<Self::Validator>> {
        None
    }

    /// Sign the given message.
    async fn sign(&self, message: Self::Message) -> Self::SignedMessage {
        let hash = message.hash();
        let mut inner = self.inner.write().unwrap();
        let signature = inner.private_key.sign(hash.as_bytes());
        SignedMessage { message, signature }
    }

    /// Validates the signature field of a message. If None returns false.
    async fn validate_signature(
        &self,
        hash: &Self::MessageHash,
        signature: &Self::Signature,
        public_key: &Self::PublicKey,
    ) -> bool {
        public_key.verify(hash.as_bytes(), &signature).is_ok()
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
        _block_hash: Self::BlockHash,
        _precommits: Vec<Self::Precommit>,
        _height: Self::Height,
    ) {
        todo!()
    }
}

fn seed_from_height(height: Height) -> [u8; 32] {
    let bytes = height.as_u64().to_le_bytes();
    let mut seed = [0; 32];
    seed.copy_from_slice(&[bytes, bytes, bytes, bytes].concat());
    seed
}
