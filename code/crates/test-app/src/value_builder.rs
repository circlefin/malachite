use std::time::Duration;

use async_trait::async_trait;

use malachite_actors::consensus::ConsensusRef;
use malachite_actors::host::{LocallyProposedValue, ReceivedProposedValue};
use malachite_common::{Context, Round, SignedVote};

#[async_trait]
pub trait ValueBuilder<Ctx: Context>: Send + Sync + 'static {
    async fn build_value_locally(
        &mut self,
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        consensus: ConsensusRef<Ctx>,
    ) -> Option<LocallyProposedValue<Ctx>>;

    async fn build_value_from_block_parts(
        &mut self,
        block_part: Ctx::BlockPart,
    ) -> Option<ReceivedProposedValue<Ctx>>;

    async fn maybe_received_value(
        &mut self,
        height: Ctx::Height,
        round: Round,
    ) -> Option<ReceivedProposedValue<Ctx>>;

    async fn decided_on_value(
        &mut self,
        height: Ctx::Height,
        round: Round,
        value: Ctx::Value,
        commits: Vec<SignedVote<Ctx>>,
    );
}
