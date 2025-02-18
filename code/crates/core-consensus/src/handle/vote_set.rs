use crate::handle::driver::apply_driver_input;
use crate::handle::vote::on_vote;
use crate::input::RequestId;
use crate::prelude::*;

pub async fn on_vote_set_request<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    request_id: RequestId,
    height: Ctx::Height,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(%height, %round, %request_id, "Received vote set request, retrieve the votes and send response if set is not empty");

    if let Some((votes, polka_certificate)) = state.restore_votes(height, round) {
        perform!(
            co,
            Effect::SendVoteSetResponse(
                request_id,
                height,
                round,
                VoteSet::new(votes),
                polka_certificate,
                Default::default()
            )
        );
    }

    Ok(())
}

pub async fn on_vote_set_response<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    vote_set: VoteSet<Ctx>,
    polka_certificate: Option<PolkaCertificate<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!(
        height = %state.height(), round = %state.round(),
        votes.count = %vote_set.len(),
        polka.certificate = %polka_certificate.is_some(),
        "Received vote set response"
    );

    if let Some(certificate) = polka_certificate {
        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::PolkaCertificate(certificate),
        )
        .await?;
    } else {
        for vote in vote_set.votes {
            on_vote(co, state, metrics, vote).await?;
        }
    }

    Ok(())
}
