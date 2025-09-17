mod decide;
mod driver;
mod liveness;
mod proposal;
mod propose;
mod proposed_value;
mod rebroadcast_timeout;
mod signature;
mod start_height;
mod sync;
mod timeout;
mod vote;

use liveness::{on_polka_certificate, on_round_certificate};
use proposal::on_proposal;
use propose::on_propose;
use proposed_value::on_proposed_value;
use start_height::reset_and_start_height;
use sync::on_value_response;
use timeout::on_timeout_elapsed;
use vote::on_vote;

use crate::prelude::*;

#[allow(private_interfaces)]
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
        Input::StartHeight(height, validator_set, is_restart) => {
            reset_and_start_height(co, state, metrics, height, validator_set, is_restart).await
        }
        Input::Vote(vote) => on_vote(co, state, metrics, vote).await,
        Input::Proposal(proposal) => on_proposal(co, state, metrics, proposal).await,
        Input::Propose(value) => on_propose(co, state, metrics, value).await,
        Input::TimeoutElapsed(timeout) => on_timeout_elapsed(co, state, metrics, timeout).await,
        Input::ProposedValue(value, origin) => {
            on_proposed_value(co, state, metrics, value, origin).await
        }
        Input::SyncValueResponse(value) => on_value_response(co, state, metrics, value).await,
        Input::PolkaCertificate(certificate) => {
            on_polka_certificate(co, state, metrics, certificate).await
        }
        Input::RoundCertificate(certificate) => {
            on_round_certificate(co, state, metrics, certificate).await
        }
    }
}
