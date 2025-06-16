use std::time::Duration;
use tokio::time::Instant;

use bytesize::ByteSize;
use malachitebft_core_types::{CommitCertificate, Round};
use malachitebft_signing_ed25519::PrivateKey;

use crate::mempool::MempoolRef;
use crate::proposal::build_proposal_task;

use crate::types::{Address, Block, Height, MockContext, ValidatorSet};

#[derive(Copy, Clone, Debug)]
pub struct MockHostParams {
    pub max_block_size: ByteSize,
    pub time_allowance_factor: f32,
    pub exec_time_per_tx: Duration,
    pub max_retain_blocks: usize,
}

pub struct MockHost {
    pub params: MockHostParams,
    pub mempool: MempoolRef,
    pub address: Address,
    pub private_key: PrivateKey,
    pub validator_set: ValidatorSet,
}

impl MockHost {
    pub fn new(
        params: MockHostParams,
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
        }
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_new_proposal(
        &mut self,
        height: Height,
        round: Round,
        deadline: Instant,
    ) -> Result<Block, Box<dyn core::error::Error + Send + Sync>> {
        let address = self.address;
        let params = self.params;
        let mempool = self.mempool.clone();
        build_proposal_task(height, round, address, params, deadline, mempool).await
    }

    /// Update the Context about which decision has been made. It is responsible for pinging any
    /// relevant components in the node to update their states accordingly.
    ///
    /// Params:
    /// - brock_hash - The ID of the content which has been decided.
    /// - precommits - The list of precommits from the round the decision was made (both for and against).
    /// - height     - The height of the decision.
    #[tracing::instrument(skip_all, fields(height = %_certificate.height, block_hash = %_certificate.value_id))]
    pub async fn decision(&self, _certificate: CommitCertificate<MockContext>) {}
}
