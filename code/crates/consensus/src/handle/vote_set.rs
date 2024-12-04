use crate::{handle::vote::on_vote, prelude::*};
use libp2p::request_response::InboundRequestId;

pub async fn on_vote_set_request<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    _metrics: &Metrics,
    request_id: InboundRequestId,
    height: Ctx::Height,
    round: Round,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    // TODO

    debug!(%height, %round, "VS8 - consensus gets the request, builds the set and response");
    let votes: Vec<SignedVote<Ctx>> = state.restore_votes(height, round);

    if !votes.is_empty() {
        let vote_set = VoteSet::new(votes);

        perform!(
            co,
            Effect::SendVoteSetResponse(request_id, height, round, vote_set)
        );
    }
    Ok(())
}

pub async fn on_vote_set_response<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    vote_set: VoteSet<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    debug!("VS99 - consensus gets the vote set response, processes votes");

    for vote in vote_set.vote_set {
        let _ = on_vote(co, state, metrics, vote).await;
    }

    Ok(())
}
