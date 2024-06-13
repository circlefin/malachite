#![allow(unused_variables)]

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr};
use tokio::time::Instant;

use malachite_actors::consensus::ConsensusMsg;
use malachite_actors::host::{LocallyProposedValue, ReceivedProposedValue};
use malachite_common::{Round, TransactionBatch, Validity};

use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::mock::part_store::PartStore;
use crate::mock::types::{Address, BlockPart, Content, Height, ProposalPart, ValidatorSet};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
}

pub struct HostState {
    part_store: PartStore<MockContext>,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

impl StarknetHost {
    pub fn new(host: MockHost) -> Self {
        Self { host }
    }

    pub fn build_content_from_block_parts(
        &self,
        state: &mut HostState,
        height: Height,
        round: Round,
    ) -> Option<(Content, Address)> {
        let block_parts = state.part_store.all_parts(height, round);

        if block_parts.is_empty() {
            return None;
        }

        let last_part = block_parts.last().expect("block_parts is not empty");

        let mut metadata = None;
        let mut tx_batches = Vec::new();

        for block_part in &block_parts {
            match block_part.part.as_ref() {
                ProposalPart::TxBatch(_, tx_batch) => {
                    tx_batches.extend(tx_batch.transactions().iter().cloned())
                }
                ProposalPart::Metadata(_, meta) => metadata = Some(meta.clone()),
            }
        }

        metadata.map(|metadata| {
            (
                Content {
                    tx_batch: TransactionBatch::new(tx_batches),
                    metadata,
                },
                last_part.validator_address,
            )
        })
    }

    pub fn build_proposed_value_from_block_parts(
        &self,
        state: &mut HostState,
        height: Height,
        round: Round,
    ) -> Option<ReceivedProposedValue<MockContext>> {
        let (value, validator_address) =
            self.build_content_from_block_parts(state, height, round)?;

        Some(ReceivedProposedValue {
            validator_address,
            height,
            round,
            value,
            valid: Validity::Valid, // TODO: Check validity
        })
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
                reply,
            } => {
                let deadline = Instant::now() + timeout_duration;

                let (mut rx_part, rx_hash) =
                    self.host.build_new_proposal(height, round, deadline).await;

                while let Some(part) = rx_part.recv().await {
                    let block_part = BlockPart::new(height, round, part.sequence(), address, part);
                    state.part_store.store(block_part.clone());

                    consensus.cast(ConsensusMsg::BuilderBlockPart(block_part))?;
                }

                // Wait until we receive the block hash, even if we have no use for it yet.
                let _block_hash = rx_hash.await?;

                if let Some((value, _)) = self.build_content_from_block_parts(state, height, round)
                {
                    let proposed_value = LocallyProposedValue::new(height, round, value);
                    reply.send(proposed_value)?;
                }

                Ok(())
            }

            HostMsg::BlockPart {
                block_part,
                reply_to,
            } => todo!(),

            HostMsg::GetReceivedValue {
                height,
                round,
                reply_to,
            } => {
                let proposed_value =
                    self.build_proposed_value_from_block_parts(state, height, round);
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

            HostMsg::DecidedOnValue { .. } => {
                todo!()
            }
        }
    }
}
