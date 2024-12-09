use std::path::Path;
use std::sync::Arc;

use rand::RngCore;
use sha3::Digest;
use tracing::{debug, error, trace};

use malachite_actors::consensus::ConsensusRef;
use malachite_actors::host::ProposedValue;
use malachite_actors::util::streaming::StreamId;
use malachite_common::{Round, SignedExtension, Validity};

use crate::block_store::BlockStore;
use crate::host::proposal::compute_proposal_hash;
use crate::host::{Host, StarknetHost};
use crate::streaming::PartStreamsMap;
use crate::types::*;

pub struct HostState {
    pub height: Height,
    pub round: Round,
    pub proposer: Option<Address>,
    pub host: StarknetHost,
    pub consensus: Option<ConsensusRef<MockContext>>,
    pub block_store: BlockStore,
    pub part_streams_map: PartStreamsMap,
    pub next_stream_id: StreamId,
}

impl HostState {
    pub fn new<R>(host: StarknetHost, db_path: impl AsRef<Path>, rng: &mut R) -> Self
    where
        R: RngCore,
    {
        Self {
            height: Height::new(0, 0),
            round: Round::Nil,
            proposer: None,
            host,
            consensus: None,
            block_store: BlockStore::new(db_path).unwrap(),
            part_streams_map: PartStreamsMap::default(),
            next_stream_id: rng.next_u64(),
        }
    }

    pub fn next_stream_id(&mut self) -> StreamId {
        let stream_id = self.next_stream_id;
        // Wrap around if we get to u64::MAX, which may happen if the initial
        // stream id was close to it already.
        self.next_stream_id = self.next_stream_id.wrapping_add(1);
        stream_id
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_value_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<ProposedValue<MockContext>> {
        let (valid_round, value, validator_address, validity, extension) = self
            .build_proposal_content_from_parts(parts, height, round)
            .await?;

        Some(ProposedValue {
            validator_address,
            height,
            round,
            valid_round,
            value,
            validity,
            extension,
        })
    }

    #[allow(clippy::type_complexity)]
    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub async fn build_proposal_content_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<(
        Round,
        BlockHash,
        Address,
        Validity,
        Option<SignedExtension<MockContext>>,
    )> {
        if parts.is_empty() {
            return None;
        }

        let Some(init) = parts.iter().find_map(|part| part.as_init()) else {
            error!("No Init part found in the proposal parts");
            return None;
        };

        let valid_round = init.valid_round;
        if valid_round.is_defined() {
            debug!("Reassembling a Proposal we might have seen before: {init:?}");
        }

        let Some(fin) = parts.iter().find_map(|part| part.as_fin()) else {
            error!("No Fin part found in the proposal parts");
            return None;
        };

        trace!(parts.len = %parts.len(), "Building proposal content from parts");

        let extension = self.host.generate_vote_extension(height, round);

        let block_hash = {
            let mut block_hasher = sha3::Keccak256::new();
            for part in parts {
                if part.as_init().is_some() || part.as_fin().is_some() {
                    // NOTE: We do not hash over Init, so restreaming returns the same hash
                    // NOTE: We do not hash over Fin, because Fin includes a signature over the block hash
                    // TODO: we should probably still include height
                    continue;
                }

                block_hasher.update(part.to_sign_bytes());
            }

            BlockHash::new(block_hasher.finalize().into())
        };

        trace!(%block_hash, "Computed block hash");

        let proposal_hash = compute_proposal_hash(init, &block_hash);

        let validity = self
            .verify_proposal_validity(init, &proposal_hash, &fin.signature)
            .await?;

        Some((
            valid_round,
            block_hash,
            init.proposer.clone(),
            validity,
            extension,
        ))
    }

    async fn verify_proposal_validity(
        &self,
        init: &ProposalInit,
        proposal_hash: &Hash,
        signature: &Signature,
    ) -> Option<Validity> {
        let validators = self.host.validators(init.height).await?;

        let public_key = validators
            .iter()
            .find(|v| v.address == init.proposer)
            .map(|v| v.public_key);

        let Some(public_key) = public_key else {
            error!(proposer = %init.proposer, "No validator found for the proposer");
            return None;
        };

        let valid = public_key.verify(&proposal_hash.as_felt(), signature);
        Some(Validity::from_bool(valid))
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

        if let ProposalPart::Transactions(_txes) = &part {
            debug!("Simulating tx execution and proof verification");

            // Simulate Tx execution and proof verification (assumes success)
            // TODO: Add config knob for invalid blocks
            let num_txes = part.tx_count() as u32;
            let exec_time = self.host.params.exec_time_per_tx * num_txes;
            tokio::time::sleep(exec_time).await;

            trace!("Simulation took {exec_time:?} to execute {num_txes} txes");
        }

        let parts = self.host.part_store.all_parts(height, round);

        trace!(
            count = self.host.part_store.blocks_count(),
            "Blocks for which we have parts"
        );

        // TODO: Do more validations, e.g. there is no higher tx proposal part,
        //       check that we have received the proof, etc.
        let Some(_fin) = parts.iter().find_map(|part| part.as_fin()) else {
            debug!("Final proposal part has not been received yet");
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
