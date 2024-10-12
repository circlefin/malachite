use std::collections::BTreeMap;

use malachite_common::NilOrVal::Val;
use malachite_common::{Certificate, Context, Proposal, SignedProposal, SignedVote, ValueId, Vote};
use malachite_starknet_p2p_types::{Transaction, Transactions};

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Block<Ctx: Context> {
    pub height: Ctx::Height,
    pub transactions: Transactions,
    pub block_id: ValueId<Ctx>,
}

#[derive(Clone, Debug)]
pub struct DecidedBlock<Ctx: Context> {
    pub block: Block<Ctx>,
    pub proposal: SignedProposal<Ctx>,
    pub certificate: Certificate<Ctx>,
}

// This is a temporary store implementation for blocks
type Store<Ctx> = BTreeMap<<Ctx as Context>::Height, DecidedBlock<Ctx>>;

#[derive(Clone, Debug)]
pub struct BlockStore<Ctx: Context> {
    pub(crate) store: Store<Ctx>,
}

impl<Ctx: Context> Default for BlockStore<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Ctx: Context> BlockStore<Ctx> {
    pub fn new() -> Self {
        Self {
            store: Default::default(),
        }
    }

    pub fn store_keys(&self) -> Vec<Ctx::Height> {
        self.store.keys().copied().collect()
    }

    pub fn store(
        &mut self,
        proposal: &SignedProposal<Ctx>,
        txes: &[Transaction],
        commits: &Vec<SignedVote<Ctx>>,
    ) {
        let block_id = match commits[0].message.value() {
            Val(h) => h,
            _ => return,
        };
        let certificate = Certificate {
            commits: commits.to_owned(),
        };
        let decided_block = DecidedBlock {
            block: Block {
                height: proposal.height(),
                block_id: block_id.clone(),
                transactions: Transactions::new(txes.to_vec()),
            },
            proposal: proposal.clone(),
            certificate,
        };

        let _ = self.store.insert(proposal.height(), decided_block);
    }

    pub fn prune(&mut self, retain_height: Ctx::Height) {
        self.store.retain(|height, _| *height >= retain_height);
    }
}
