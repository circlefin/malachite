#![allow(unused_variables)]

use malachite_actors::consensus::ConsensusMsg;
use malachite_actors::host::LocallyProposedValue;
use malachite_common::TransactionBatch;
use ractor::{async_trait, Actor, ActorProcessingErr};
use tokio::time::Instant;

use crate::mock::context::MockContext;
use crate::mock::host::MockHost;
use crate::mock::part_store::PartStore;
use crate::mock::types::{BlockPart, Content, ProposalPart};
use crate::Host;

pub struct StarknetHost {
    host: MockHost,
}

pub struct HostState {
    part_store: PartStore<MockContext>,
}

pub type HostRef = malachite_actors::host::HostRef<MockContext>;
pub type HostMsg = malachite_actors::host::HostMsg<MockContext>;

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

                let mut tx_batch = Vec::new();
                let mut metadata = None;

                while let Some(part) = rx_part.recv().await {
                    match &part {
                        ProposalPart::TxBatch(_, batch) => {
                            tx_batch.extend(batch.transactions().iter().cloned());
                        }
                        ProposalPart::Metadata(_, meta) => {
                            metadata = Some(meta.clone());
                        }
                    }

                    let block_part = BlockPart::new(height, round, part.sequence(), address, part);
                    state.part_store.store(block_part.clone());

                    consensus.cast(ConsensusMsg::BuilderBlockPart(block_part))?;
                }

                // Wait until we receive the block hash, even if we have no use for it yet.
                let _block_hash = rx_hash.await?;

                let value = metadata.map(|metadata| Content {
                    tx_batch: TransactionBatch::new(tx_batch),
                    metadata,
                });

                let proposed_value = LocallyProposedValue {
                    height,
                    round,
                    value,
                };

                reply.send(proposed_value)?;

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
            } => todo!(),

            HostMsg::GetValidatorSet { height, reply_to } => todo!(),
        }
    }
}
