use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rand::RngCore;
use tracing::{debug, error, trace};

use malachitebft_core_types::{Context, Round, Validity};
use malachitebft_engine::consensus::ConsensusRef;
use malachitebft_engine::host::ProposedValue;
use malachitebft_engine::util::streaming::StreamId;
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::block_store::BlockStore;
use crate::host::{Host, StarknetHost};
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

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_value_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<ProposedValue<MockContext>> {
        let (valid_round, value, proposer, validity) = self
            .build_proposal_content_from_parts(parts, height, round)
            .await?;

        Some(ProposedValue {
            proposer,
            height,
            round,
            valid_round,
            value,
            validity,
        })
    }

    #[allow(clippy::type_complexity)]
    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_proposal_content_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<(Round, BlockHash, Address, Validity)> {
        if parts.is_empty() {
            return None;
        }

        let Some(init) = parts.iter().find_map(|part| part.as_init()) else {
            error!("Part not found: Init");
            return None;
        };

        let Some(fin) = parts.iter().find_map(|part| part.as_fin()) else {
            error!("Part not found: Fin");
            return None;
        };

        let Some(_block_info) = parts.iter().find_map(|part| part.as_block_info()) else {
            error!("Part not found: BlockInfo");
            return None;
        };

        let Some(commitment) = parts.iter().find_map(|part| part.as_commitment()) else {
            error!("Part not found: ProposalCommitment");
            return None;
        };

        let validity = self.verify_proposal_validity(init, fin, commitment).await?;

        let valid_round = init.valid_round;
        if valid_round.is_defined() {
            debug!("Reassembling a proposal we might have seen before: {init:?}");
        }

        trace!(parts.len = %parts.len(), "Building proposal content from parts");

        Some((
            valid_round,
            fin.proposal_commitment_hash,
            init.proposer,
            validity,
        ))
    }

    async fn verify_proposal_validity(
        &self,
        init: &ProposalInit,
        _fin: &ProposalFin,
        _commitment: &ProposalCommitment,
    ) -> Option<Validity> {
        let validators = self.host.validators(init.height).await?;

        if !validators.iter().any(|v| v.address == init.proposer) {
            error!(proposer = %init.proposer, "No validator found for the proposer");
            return None;
        };

        let validator_set = ValidatorSet::new(validators);
        let proposer = self
            .ctx
            .select_proposer(&validator_set, init.height, init.round);

        if proposer.address != init.proposer {
            error!(
                height = %init.height,
                round = %init.round,
                proposer = %init.proposer,
                expected = %proposer.address,
                "Proposer is not the selected proposer for this height and round"
            );

            return None;
        }

        // TODO: Check that the hash of `commitment` matches `fin.proposal_commitment_hash`

        Some(Validity::Valid)
    }

    #[tracing::instrument(skip_all, fields(
        part.height = %height,
        part.round = %round,
        part.message = ?part.part_type(),
    ))]
    pub async fn build_value_from_part(
        &mut self,
        height: Height,
        round: Round,
        part: ProposalPart,
    ) -> Option<ProposedValue<MockContext>> {
        self.host.part_store.store(height, round, part.clone());

        if let ProposalPart::Transactions(txes) = &part {
            if self.host.params.exec_time_per_tx > Duration::from_secs(0) {
                debug!("Simulating tx execution and proof verification");

                // Simulate Tx execution and proof verification (assumes success)
                // TODO: Add config knob for invalid blocks
                let num_txes = txes.len() as u32;
                let exec_time = self.host.params.exec_time_per_tx * num_txes;
                tokio::time::sleep(exec_time).await;

                trace!("Simulation took {exec_time:?} to execute {num_txes} txes");
            }
        }

        let parts = self.host.part_store.all_parts(height, round);

        trace!(
            count = self.host.part_store.blocks_count(),
            "Blocks for which we have parts"
        );

        // TODO: Do more validations, e.g. there is no higher tx proposal part,
        //       check that we have received the proof, etc.
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

        let result = self.build_value_from_parts(&parts, height, round).await;

        if let Some(ref proposed_value) = result {
            self.host
                .part_store
                .store_value_id(height, round, proposed_value.value);
        }

        result
    }
}
