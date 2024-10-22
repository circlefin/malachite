use crate::prelude::*;

mod decide;
mod driver;
mod proposal;
mod propose_value;
mod received_proposed_value;
mod signature;
mod start_height;
mod synced_block;
mod timeout;
mod validator_set;
mod vote;

use proposal::on_proposal;
use propose_value::propose_value;
use received_proposed_value::on_received_proposed_value;
use start_height::reset_and_start_height;
use synced_block::on_received_synced_block;
use timeout::on_timeout_elapsed;
use vote::on_vote;

pub async fn handle<Ctx>(
    co: Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    input: Input<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    handle_input(&co, state, metrics, input).await
}

#[async_recursion]
async fn handle_input<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    input: Input<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    match input {
        Input::StartHeight(height, validator_set) => {
            reset_and_start_height(co, state, metrics, height, validator_set).await
        }
        Input::Vote(vote) => on_vote(co, state, metrics, vote).await,
        Input::Proposal(proposal) => on_proposal(co, state, metrics, proposal).await,
        Input::ProposeValue(height, round, value, extension) => {
            propose_value(co, state, metrics, height, round, value, extension).await
        }
        Input::TimeoutElapsed(timeout) => on_timeout_elapsed(co, state, metrics, timeout).await,
        Input::ReceivedProposedValue(value) => {
            on_received_proposed_value(co, state, metrics, value).await
        }
        Input::ReceivedSyncedBlock(proposal, commits, block_bytes) => {
            on_received_synced_block(co, state, metrics, proposal, commits, block_bytes).await
        }
    }
}
