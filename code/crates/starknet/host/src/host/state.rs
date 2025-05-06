use sha3::Digest;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rand::RngCore;
use tracing::{debug, error, trace};

use malachitebft_core_types::{Round, Validity};
use malachitebft_engine::consensus::ConsensusRef;
use malachitebft_engine::host::ProposedValue;
use malachitebft_engine::util::streaming::StreamId;
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::block_store::BlockStore;
use crate::host::StarknetHost;
use crate::streaming::PartStreamsMap;
use crate::types::*;

pub struct HostState {
    pub ctx: MockContext,
    pub height: Height,
    pub round: Round,
    pub proposer: Option<Address>,
    pub host: StarknetHost,
    pub consensus: Option<ConsensusRef<MockContext>>,
    pub block_store: BlockStore,
    pub part_streams_map: PartStreamsMap,
    pub nonce: u64,
}

impl HostState {
    pub fn new<R>(
        ctx: MockContext,
        host: StarknetHost,
        db_path: impl AsRef<Path>,
        rng: &mut R,
    ) -> Self
    where
        R: RngCore,
    {
        Self {
            ctx,
            height: Height::new(0, 0),
            round: Round::Nil,
            proposer: None,
            host,
            consensus: None,
            block_store: BlockStore::new(db_path).unwrap(),
            part_streams_map: PartStreamsMap::default(),
            nonce: rng.next_u64(),
        }
    }

    pub fn stream_id(&mut self) -> StreamId {
        let stream_id = p2p_proto::ConsensusStreamId {
            height: self.height.as_u64(),
            round: self.round.as_u32().expect("round is non-nil"),
            nonce: self.nonce,
        };

        self.nonce += 1;

        let bytes = prost::Message::encode_to_vec(&stream_id);
        StreamId::new(bytes.into())
    }

    #[allow(clippy::type_complexity)]
    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_proposal_from_parts(
        &self,
        height: Height,
        round: Round,
        parts: &[Arc<ProposalPart>],
    ) -> ProposedValue<MockContext> {
        // We must be here with non-empty `parts`, must have init, fin, commitment and maybe transactions
        assert!(!parts.is_empty(), "Parts must not be empty");

        let init = parts
            .iter()
            .find_map(|part| part.as_init())
            .expect("Init part not found");

        let fin = parts
            .iter()
            .find_map(|part| part.as_fin())
            .expect("Fin part not found");

        let _block_info = parts
            .iter()
            .find_map(|part| part.as_block_info())
            .expect("BlockInfo part not found");

        let commitment = parts
            .iter()
            .find_map(|part| part.as_commitment())
            .expect("ProposalCommitment part not found");

        // Collect all transactions from the transaction parts
        // We expect that the transaction parts are ordered by sequence number but we don't have a way to check
        // this here, so we just collect them in the order.
        let transactions: Vec<Transaction> = parts
            .iter()
            .filter_map(|part| part.as_transactions())
            .flat_map(|batch| batch.as_slice().iter().cloned())
            .collect();

        // Determine the validity of the proposal
        let validity = self
            .verify_proposal_validity(fin, commitment, transactions)
            .await;

        let valid_round = init.valid_round;
        if valid_round.is_defined() {
            debug!("Reassembling a proposal we might have seen before: {init:?}");
        }

        trace!(parts.len = %parts.len(), "Building proposal content from parts");

        ProposedValue {
            proposer: init.proposer,
            height,
            round,
            valid_round,
            value: fin.proposal_commitment_hash,
            validity,
        }
    }

    async fn verify_proposal_validity(
        &self,
        fin: &ProposalFin,
        commitment: &ProposalCommitment,
        transactions: Vec<Transaction>,
    ) -> Validity {
        let mut hasher = sha3::Keccak256::new();

        for tx in transactions.iter() {
            hasher.update(tx.hash().as_bytes());
        }

        let transaction_commitment = Hash::new(hasher.finalize().into());

        // TODO: Check that computed transaction_commitment and state_diff_commitment match the ones in the `commitment` and
        // the propposal commitment hash matches `fin.proposal_commitment_hash`
        // For now we just check that the hash of transactions matches the transaction commitment in `commitment`
        // and that the proposal commitment hash matches `fin.proposal_commitment_hash`
        let valid_proposal = transaction_commitment == commitment.transaction_commitment
            && transaction_commitment == fin.proposal_commitment_hash;

        if valid_proposal {
            Validity::Valid
        } else {
            error!(
                "ProposalCommitment hash mismatch: {} != {}",
                transaction_commitment, fin.proposal_commitment_hash
            );
            Validity::Invalid
        }
    }

    #[tracing::instrument(skip_all, fields(
        part.height = %height,
        part.round = %round,
        part.message = ?part.part_type(),
    ))]
    pub async fn build_value_from_part(
        &mut self,
        stream_id: &StreamId,
        height: Height,
        round: Round,
        part: ProposalPart,
    ) -> Option<ProposedValue<MockContext>> {
        self.host
            .part_store
            .store(stream_id, height, round, part.clone());

        if let ProposalPart::Transactions(txes) = &part {
            if self.host.params.exec_time_per_tx > Duration::from_secs(0) {
                debug!("Simulating tx execution and proof verification");

                // Simulate Tx execution. In the real implementation the results of the execution would be
                // accumulated in some intermediate state structure based on which the proposal commitment
                // will be computed once all parts are received and checked against the received
                // `ProposalCommitment` part (e.g. `state_diff_commitment`) and the `proposal_commitment_hash`
                // in the `Fin` part.
                let num_txes = txes.len() as u32;
                let exec_time = self.host.params.exec_time_per_tx * num_txes;
                tokio::time::sleep(exec_time).await;

                trace!("Simulation took {exec_time:?} to execute {num_txes} txes");
            }
        }

        let parts = self
            .host
            .part_store
            .all_parts_by_stream_id(stream_id.clone(), height, round);

        trace!(
            count = self.host.part_store.blocks_count(),
            "Blocks for which we have parts"
        );

        // TODO: Do more validations, e.g. there is no higher tx proposal part,
        // check that we have received the proof, etc.
        let Some(_fin) = parts.iter().find_map(|part| part.as_fin()) else {
            debug!("Proposal part has not been received yet: Fin");
            return None;
        };

        let Some(_block_info) = parts.iter().find_map(|part| part.as_block_info()) else {
            debug!("Proposal part has not been received yet: BlockInfo");
            return None;
        };

        let Some(_proposal_commitment) = parts.iter().find_map(|part| part.as_commitment()) else {
            debug!("Proposal part has not been received yet: ProposalCommitment");
            return None;
        };

        let block_size: usize = parts.iter().map(|p| p.size_bytes()).sum();
        let tx_count: usize = parts.iter().map(|p| p.tx_count()).sum();

        debug!(
            tx.count = %tx_count, block.size = %block_size, parts.count = %parts.len(),
            "All parts have been received already, building value"
        );

        // TODO: Add config knob for invalid blocks
        let proposed_value = self.build_proposal_from_parts(height, round, &parts).await;

        self.host
            .part_store
            .store_value_id(stream_id, height, round, proposed_value.value);

        Some(proposed_value)
    }
}
