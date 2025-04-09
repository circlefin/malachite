use crate::prelude::*;

pub async fn sign_vote<Ctx>(co: &Co<Ctx>, vote: Ctx::Vote) -> Result<SignedVote<Ctx>, Error<Ctx>>
where
    Ctx: Context,
{
    let signed_vote = perform!(co,
        Effect::SignVote(vote, Default::default()),
        Resume::SignedVote(signed_vote) => signed_vote
    );

    Ok(signed_vote)
}

pub async fn sign_proposal<Ctx>(
    co: &Co<Ctx>,
    proposal: Ctx::Proposal,
) -> Result<SignedProposal<Ctx>, Error<Ctx>>
where
    Ctx: Context,
{
    let signed_proposal = perform!(co,
        Effect::SignProposal(proposal, Default::default()),
        Resume::SignedProposal(signed_proposal) => signed_proposal
    );

    Ok(signed_proposal)
}
