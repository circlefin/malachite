#![allow(unused_variables, unused_imports)]

use std::sync::Arc;

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr, SpawnErr};
use sha3::Digest;
use tokio::time::Instant;
use tracing::{debug, error, trace};

use malachite_actors::consensus::ConsensusMsg;
use malachite_actors::host::{LocallyProposedValue, ProposedValue};
use malachite_common::{Round, Validity};
use malachite_metrics::Metrics;
use malachite_starknet_p2p_types::{Proposal, ProposalMessage};

use crate::mempool::{MempoolMsg, MempoolRef};
use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::part_store::PartStore;
use crate::types::{Address, BlockHash, Height, ProposalPart, ValidatorSet};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
    mempool: MempoolRef,
    metrics: Metrics,
}

#[derive(Default)]
pub struct HostState {
    part_store: PartStore<MockContext>,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl StarknetHost {
    pub fn new(host: MockHost, mempool: MempoolRef, metrics: Metrics) -> Self {
        Self {
            host,
            mempool,
            metrics,
        }
    }

    pub async fn spawn(
        host: MockHost,
        mempool: MempoolRef,
        metrics: Metrics,
    ) -> Result<HostRef, SpawnErr> {
        let (actor_ref, _) = Actor::spawn(
            None,
            Self::new(host, mempool, metrics),
            HostState::default(),
        )
        .await?;

        Ok(actor_ref)
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub fn build_value_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<ProposedValue<MockContext>> {
        let (value, validator_address, validity) =
            self.build_proposal_content_from_parts(parts, height, round)?;

        Some(ProposedValue {
            validator_address,
            height,
            round,
            value,
            validity,
        })
    }

    #[tracing::instrument(skip_all, fields(%height, %round))]
    pub fn build_proposal_content_from_parts(
        &self,
        parts: &[Arc<ProposalPart>],
        height: Height,
        round: Round,
    ) -> Option<(BlockHash, Address, Validity)> {
        if parts.is_empty() {
            return None;
        }

        debug!(parts.len = %parts.len(), "Building proposal content from parts");

        let block_hash = {
            let mut block_hasher = sha3::Keccak256::new();
            for part in parts {
                block_hasher.update(part.to_sign_bytes());
            }
            BlockHash::new(block_hasher.finalize().into())
        };

        debug!(%block_hash, "Computed block hash");

        let last_part = parts.last().expect("proposal_parts is not empty");

        // TODO: How to compute validity?
        //
        // let Some(block_hash) = last_part.block_hash() else {
        //     error!("Block is missing metadata");
        //     return None;
        // };
        //
        // let expected_value = BlockHash::new(block_hasher.finalize().into());
        //
        // let valid = if block_hash != expected_value {
        //     error!("Invalid block received with value {block_hash}, expected {expected_value}");
        //     Validity::Invalid
        // } else {
        //     Validity::Valid
        // };

        Some((block_hash, last_part.validator.clone(), Validity::Valid))
    }

    #[tracing::instrument(skip_all, fields(
        %part.height,
        %part.round,
        %part.sequence,
        part.message = ?part.message_type(),
    ))]
    async fn build_value_from_part(
        &self,
        state: &mut HostState,
        part: ProposalPart,
    ) -> Option<ProposedValue<MockContext>> {
        let height = part.height;
        let round = part.round;
        let sequence = part.sequence;

        debug!("Received proposal part");

        // Prune all proposal parts for heights lower than `height - 1`
        state.part_store.prune(height.decrement().unwrap_or(height));
        state.part_store.store(part.clone());

        if let ProposalMessage::Transactions(txes) = &part.message {
            debug!("Simulating tx execution and proof verification");

            // Simulate Tx execution and proof verification (assumes success)
            // TODO: Add config knob for invalid blocks
            let num_txes = part.tx_count() as u32;
            let exec_time = self.host.params().exec_time_per_tx * num_txes;
            tokio::time::sleep(exec_time).await;

            debug!("Simulation took {exec_time:?} to execute {num_txes} txes");
        }

        // Get the "last" part, the one with highest sequence.
        // Block parts may not be received in order.
        let all_parts = state.part_store.all_parts(height, round);
        let last_part = all_parts.last().expect("all_parts is not empty");

        // Check if the part with the highest sequence number is a `Fin` message.
        // Otherwise abort, and wait for this part to be received.
        // TODO: Do more validations, e.g. there is no higher tx proposal part,
        //       check that we have received the proof, etc.
        let ProposalMessage::Fin(_) = &last_part.message else {
            debug!("Final proposal part has not been received yet");
            return None;
        };

        let num_parts = all_parts.len();

        if num_parts == last_part.sequence as usize {
            let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
            let tx_count: usize = all_parts.iter().map(|p| p.tx_count()).sum();

            debug!(%tx_count, %block_size, %num_parts, "All parts have been received already, building value");

            self.build_value_from_parts(&all_parts, height, round)
        } else {
            trace!("Not all parts have been received yet");
            None
        }
    }
}

#[async_trait]
impl Actor for StarknetHost {
    type Arguments = HostState;
    type State = HostState;
    type Msg = HostMsg;

    async fn pre_start(
        &self,
        _myself: HostRef,
        initial_state: Self::State,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(initial_state)
    }

    #[tracing::instrument("starknet.host", skip_all)]
    async fn handle(
        &self,
        _myself: HostRef,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            HostMsg::GetValue {
                height,
                round,
                timeout_duration,
                consensus,
                address,
                reply_to,
            } => {
                let deadline = Instant::now() + timeout_duration;

                debug!(%height, %round, "Building new proposal...");
                let (mut rx_part, rx_hash) =
                    self.host.build_new_proposal(height, round, deadline).await;

                while let Some(part) = rx_part.recv().await {
                    state.part_store.store(part.clone());
                    consensus.cast(ConsensusMsg::GossipProposalPart(part))?;
                }

                let block_hash = rx_hash.await?;
                debug!("Got block with hash: {block_hash}");

                let parts = state.part_store.all_parts(height, round);

                if let Some(value) = self.build_value_from_parts(&parts, height, round) {
                    reply_to.send(LocallyProposedValue::new(
                        value.height,
                        value.round,
                        value.value,
                    ))?;
                }

                Ok(())
            }

            HostMsg::ReceivedProposalPart { part, reply_to } => {
                if let Some(value) = self.build_value_from_part(state, part).await {
                    reply_to.send(value)?;
                }

                Ok(())
            }

            HostMsg::GetReceivedValue {
                height,
                round,
                reply_to,
            } => {
                let proposal_parts = state.part_store.all_parts(height, round);
                let proposed_value = self.build_value_from_parts(&proposal_parts, height, round);
                reply_to.send(proposed_value)?;

                Ok(())
            }

            HostMsg::GetValidatorSet { height, reply_to } => {
                if let Some(validators) = self.host.validators(height).await {
                    reply_to.send(ValidatorSet::new(validators))?;
                    Ok(())
                } else {
                    Err(eyre!("No validator set found for the given height {height}").into())
                }
            }

            HostMsg::DecidedOnValue {
                height,
                round,
                value: block_hash,
                commits,
            } => {
                let all_parts = state.part_store.all_parts(height, round);

                // TODO: Build the block from proposal parts and commits and store it

                // Update metrics
                let block_size: usize = all_parts.iter().map(|p| p.size_bytes()).sum();
                let tx_count: usize = all_parts.iter().map(|p| p.tx_count()).sum();

                self.metrics.block_tx_count.observe(tx_count as f64);
                self.metrics.block_size_bytes.observe(block_size as f64);
                self.metrics.finalized_txes.inc_by(tx_count as u64);

                // Send Update to mempool to remove all the tx-es included in the block.
                let mut tx_hashes = vec![];
                for part in all_parts {
                    if let ProposalMessage::Transactions(txes) = &part.as_ref().message {
                        tx_hashes.extend(txes.as_slice().iter().map(|tx| tx.hash()));
                    }
                }

                // Prune the PartStore of all parts for heights lower than `height - 1`
                state.part_store.prune(height.decrement().unwrap_or(height));

                // Notify the mempool to remove corresponding txs
                self.mempool.cast(MempoolMsg::Update { tx_hashes })?;

                // Notify Starknet Host of the decision
                self.host.decision(block_hash, commits, height).await;

                Ok(())
            }
        }
    }
}
