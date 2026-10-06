use crate::prelude::*;

use crate::types::ConsensusMsg;
use crate::wal_replay_index::{votes_diverge, RecordKind};

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

/// Sign a vote, unless the write-ahead log already holds the vote we cast for this
/// `(kind, height, round)` — in which case publish that one instead.
///
/// During replay:
///
/// - No record: the crash caught us before this vote reached the log. Sign and send.
/// - Record agrees: reuse the recorded signature.
/// - Record disagrees: re-derivation wasn't deterministic. Fail, and stop the node.
///
/// Outside of replay the index is empty and this is exactly [`sign_vote`].
pub async fn sign_or_replay_vote<Ctx>(
    co: &Co<Ctx>,
    state: &State<Ctx>,
    vote: Ctx::Vote,
) -> Result<SignedVote<Ctx>, Error<Ctx>>
where
    Ctx: Context,
{
    let Some(recorded) = state.replay_index.recorded_vote(&vote) else {
        return sign_vote(co, vote).await;
    };

    let kind = RecordKind::of_vote(vote.vote_type());

    if votes_diverge::<Ctx>(&vote, &recorded.message) {
        error!(
            %kind,
            height = %vote.height(),
            round = %vote.round(),
            derived = ?vote,
            recorded = ?recorded.message,
            "Re-derived vote does not match the one recorded in the write-ahead log"
        );

        return Err(Error::ReplayDivergence(kind, vote.height(), vote.round()));
    }

    info!(
        %kind,
        height = %vote.height(),
        round = %vote.round(),
        // Extensions are non-deterministic, so they may differ.
        extension_differs = vote != recorded.message,
        "Reusing the vote recorded in the write-ahead log"
    );

    Ok(recorded.clone())
}

/// Sign a proposal, unless the write-ahead log already holds the proposal we made for this
/// `(height, round)` — in which case publish that one instead.
///
/// The vote counterpart, [`sign_or_replay_vote`], documents the three cases.
pub async fn sign_or_replay_proposal<Ctx>(
    co: &Co<Ctx>,
    state: &State<Ctx>,
    proposal: Ctx::Proposal,
) -> Result<SignedProposal<Ctx>, Error<Ctx>>
where
    Ctx: Context,
{
    let Some(recorded) = state.replay_index.recorded_proposal(&proposal) else {
        return sign_proposal(co, proposal).await;
    };

    if proposal != recorded.message {
        error!(
            height = %proposal.height(),
            round = %proposal.round(),
            derived = ?proposal,
            recorded = ?recorded.message,
            "Re-derived proposal does not match the one recorded in the write-ahead log"
        );

        return Err(Error::ReplayDivergence(
            RecordKind::Proposal,
            proposal.height(),
            proposal.round(),
        ));
    }

    info!(
        height = %proposal.height(),
        round = %proposal.round(),
        "Reusing the proposal recorded in the write-ahead log"
    );

    Ok(recorded.clone())
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

pub async fn verify_extended_commit_certificate<Ctx>(
    co: &Co<Ctx>,
    certificate: ExtendedCommitCertificate<Ctx>,
    validator_set: Ctx::ValidatorSet,
    threshold_params: ThresholdParams,
    vote_extension_policy: VoteExtensionPolicy,
) -> Result<Result<(), CertificateError<Ctx>>, Error<Ctx>>
where
    Ctx: Context,
{
    let result = perform!(co,
        Effect::VerifyExtendedCommitCertificate(certificate, validator_set, threshold_params, vote_extension_policy, Default::default()),
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

pub async fn verify_round_certificate<Ctx>(
    co: &Co<Ctx>,
    certificate: RoundCertificate<Ctx>,
    validator_set: Ctx::ValidatorSet,
    threshold_params: ThresholdParams,
) -> Result<Result<(), CertificateError<Ctx>>, Error<Ctx>>
where
    Ctx: Context,
{
    let result = perform!(co,
        Effect::VerifyRoundCertificate(certificate, validator_set, threshold_params, Default::default()),
        Resume::CertificateValidity(result) => result
    );

    Ok(result)
}
