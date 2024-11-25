use crate::prelude::*;

mod decide;
mod driver;
mod proposal;
mod propose;
mod proposed_value;
mod signature;
mod start_height;
mod sync;
mod timeout;
mod validator_set;
mod vote;

use proposal::on_proposal;
use propose::on_propose;
use proposed_value::on_proposed_value;
use start_height::reset_and_start_height;
use sync::on_commit_certificate;
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
        Input::Propose(value) => on_propose(co, state, metrics, value).await,
        Input::TimeoutElapsed(timeout) => on_timeout_elapsed(co, state, metrics, timeout).await,
        Input::ProposedValue(value, origin) => {
            on_proposed_value(co, state, metrics, value, origin).await
        }
        Input::CommitCertificate(certificate) => {
            on_commit_certificate(co, state, metrics, certificate).await
        }
    }
}
