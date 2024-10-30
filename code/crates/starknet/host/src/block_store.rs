use std::collections::BTreeMap;

use malachite_common::Value;
use malachite_common::{Certificate, Proposal, SignedProposal, SignedVote};
use malachite_starknet_p2p_types::{Block, Height, Transaction, Transactions};

use crate::mock::context::MockContext;

#[derive(Clone, Debug)]
pub struct DecidedBlock {
    pub block: Block,
    pub proposal: SignedProposal<MockContext>,
    pub certificate: Certificate<MockContext>,
}

// This is a temporary store implementation for blocks
type Store = BTreeMap<Height, DecidedBlock>;

#[derive(Clone, Debug)]
pub struct BlockStore {
    pub(crate) store: Store,
}

impl Default for BlockStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BlockStore {
    pub fn new() -> Self {
        Self {
            store: Default::default(),
        }
    }

    pub fn store_keys(&self) -> impl Iterator<Item = Height> + use<'_> {
        self.store.keys().copied()
    }

    pub fn store(
        &mut self,
        proposal: &SignedProposal<MockContext>,
        txes: &[Transaction],
        commits: &[SignedVote<MockContext>],
    ) {
        let block_id = proposal.value().id();

        let certificate = Certificate {
            commits: commits.to_vec(),
        };

        let decided_block = DecidedBlock {
            block: Block {
                height: proposal.height(),
                block_hash: block_id,
                transactions: Transactions::new(txes.to_vec()),
            },
            proposal: proposal.clone(),
            certificate,
        };

        self.store.insert(proposal.height(), decided_block);
    }

    pub fn prune(&mut self, retain_height: Height) {
        self.store.retain(|height, _| *height >= retain_height);
    }
}
