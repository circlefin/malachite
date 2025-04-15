use crate::prelude::*;

use crate::types::ConsensusMsg;

pub async fn verify_signature<Ctx>(
    co: &Co<Ctx>,
    signed_msg: SignedMessage<Ctx, ConsensusMsg<Ctx>>,
    validator: &Ctx::Validator,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    let valid = perform!(co,
        Effect::VerifySignature(signed_msg, validator.public_key().clone(), Default::default()),
        Resume::SignatureValidity(valid) => valid
    );

    Ok(valid)
}

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

pub async fn verify_commit_certificate<Ctx>(
    co: &Co<Ctx>,
    certificate: CommitCertificate<Ctx>,
    validator_set: Ctx::ValidatorSet,
    threshold_params: ThresholdParams,
) -> Result<Result<(), CertificateError<Ctx>>, Error<Ctx>>
where
    Ctx: Context,
{
    let result = perform!(co,
        Effect::VerifyCommitCertificate(certificate, validator_set, threshold_params, Default::default()),
        Resume::CertificateValidity(result) => result
    );

    Ok(result)
}

pub async fn verify_polka_certificate<Ctx>(
    co: &Co<Ctx>,
    certificate: PolkaCertificate<Ctx>,
    validator_set: Ctx::ValidatorSet,
    threshold_params: ThresholdParams,
) -> Result<Result<(), CertificateError<Ctx>>, Error<Ctx>>
where
    Ctx: Context,
{
    let result = perform!(co,
        Effect::VerifyPolkaCertificate(certificate, validator_set, threshold_params, Default::default()),
        Resume::CertificateValidity(result) => result
    );

    Ok(result)
}
