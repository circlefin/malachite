use crate::handle::driver::apply_driver_input;
use crate::handle::vote::verify_vote_extension;
use crate::prelude::*;
use crate::types::ConsensusMsg;

use super::signature::{verify_polka_certificate, verify_round_certificate, verify_signature};

/// Record the equivocation evidence carried by a certificate that is not otherwise processed.
///
/// A certificate is identified by the polka it witnesses or by the round it justifies, never by
/// its signer set. Two certificates sharing an identity can therefore carry different signers,
/// and the one that is dropped may hold the only copy of a vote that conflicts with one already
/// recorded.
///
/// A signature is verified only when recording it would yield a new equivocation pair, so a
/// certificate that raises no conflict, repeats one already stored, or names a validator whose
/// evidence is already capped costs no verification at all.
async fn record_certificate_equivocations<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    votes: Vec<SignedVote<Ctx>>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    for vote in votes {
        if !state.can_record_equivocation(&vote) {
            continue;
        }

        let Some(validator) = state
            .validator_set()
            .get_by_address(vote.validator_address())
            .cloned()
        else {
            continue;
        };

        let signed_msg = vote.clone().map(ConsensusMsg::Vote);
        if !verify_signature(co, signed_msg, &validator).await? {
            continue;
        }

        if let Some((existing, conflicting)) = state.record_vote_evidence(vote) {
            warn!(
                validator = %validator.address(),
                height = %conflicting.height(),
                round = %conflicting.round(),
                existing = ?existing.value(),
                conflicting = ?conflicting.value(),
                "Recorded equivocation evidence carried by a certificate"
            );
        }
    }

    Ok(())
}

/// Handles the processing of a polka certificate.
///
/// This function is responsible for:
/// 1. Validating that the certificate's height matches the current state height
/// 2. Retrieving and verifying the validator set for the given height
/// 3. Verifying the polka certificate's validity using the validator set
/// 4. Applying the certificate to the consensus state if valid
///
/// Note: The certificate is sent to the driver as a single input to make sure a
/// `ProposalAndPolka...` input is generated and sent to the state machine
/// even in the presence of equivocating votes.
///
/// # Returns
/// * `Result<(), Error<Ctx>>` - Ok(()) if processing completed successfully (even if certificate was invalid),
///   or an error if there was a problem processing the certificate
pub async fn on_polka_certificate<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificate: PolkaCertificate<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(%certificate.height, %certificate.round, "Received polka certificate");

    // Discard certificates for heights that do not match the current height.
    if certificate.height != state.height() {
        warn!(
            %certificate.height,
            consensus.height = %state.height(),
            "Polka certificate height mismatch"
        );

        return Ok(());
    }

    // Skip certificate processing if one is already stored, to avoid redundant
    // signature verification, duplicate WAL appends, and no-op driver inputs.
    // The stored certificate is keyed by round and value alone, so salvage any
    // equivocation the skipped one witnesses before dropping it.
    if state
        .polka_certificate(certificate.round, &certificate.value_id)
        .is_some()
    {
        debug!(
            %certificate.height,
            %certificate.round,
            "Polka certificate already known, ignoring"
        );

        let votes = certificate.votes(&state.ctx).collect();
        record_certificate_equivocations(co, state, votes).await?;

        return Ok(());
    }

    let validator_set = state.validator_set();

    let validity = verify_polka_certificate(
        co,
        certificate.clone(),
        validator_set.clone(),
        state.params.threshold_params,
    )
    .await?;

    if let Err(e) = validity {
        warn!(?certificate, "Invalid polka certificate: {e}");
        return Ok(());
    }

    perform!(
        co,
        Effect::WalAppend(
            certificate.height,
            Input::PolkaCertificate(certificate.clone()),
            Default::default()
        )
    );

    apply_driver_input(
        co,
        state,
        metrics,
        DriverInput::PolkaCertificate(certificate),
    )
    .await
}

/// Handles the processing of a round certificate.
///
/// This function is responsible for:
/// 1. Validating that the certificate's height and round are eligible
/// 2. Skipping verification and WAL appends when every vote is already absorbed
///    by the vote keeper
/// 3. Verifying the certificate as a bundle
/// 4. Applying only votes not already absorbed, persisting each to the WAL
///
/// Note: The round certificate can be of type `2f+1` PrecommitAny or `f+1` SkipRound.
/// For round certificates, in contrast to polka certificates, the votes are applied
/// individually to the driver and once the threshold is reached it is sent to the state machine.
/// Presence of equivocating votes is not a problem, as the driver will ignore them while
/// the vote keeper will still be able to generate the threshold output using the existing
/// stored and incoming votes from the certificate.
///
/// # Returns
/// * `Result<(), Error<Ctx>>` - Ok(()) if processing completed successfully,
///   or an error if there was a problem processing the certificate
pub async fn on_round_certificate<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    certificate: RoundCertificate<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    info!(
        %certificate.height,
        %certificate.round,
        "Received round certificate"
    );

    // Discard certificates for heights that do not match the current height.
    if certificate.height != state.height() {
        debug!(
            %certificate.height,
            consensus.height = %state.height(),
            "Round certificate height mismatch"
        );

        return Ok(());
    }

    // A certificate that no longer advances the round carries no round to enter, but its votes
    // may still witness an equivocation, so salvage that before dropping it.
    let superseded = match certificate.cert_type {
        RoundCertificateType::Precommit => certificate.round < state.round(),
        RoundCertificateType::Skip => certificate.round <= state.round(),
    };

    if superseded {
        debug!(
            %certificate.round,
            consensus.round = %state.round(),
            ?certificate.cert_type,
            "Round certificate no longer advances the round, ignoring"
        );

        let votes = certificate.votes(&state.ctx).collect();
        record_certificate_equivocations(co, state, votes).await?;

        return Ok(());
    }

    // Skip certificate processing if an equivalent one is already stored, to avoid redundant
    // signature verification, duplicate WAL appends, and no-op driver inputs. The driver keeps
    // a single round_certificate slot; matching height/round/cert_type is sufficient because a
    // later threshold for the same (round, cert_type) would replace, not duplicate, the slot.
    // That key covers no signer, so salvage any equivocation the skipped certificate witnesses.
    let already_known = state.round_certificate().is_some_and(|existing| {
        existing.certificate.height == certificate.height
            && existing.certificate.round == certificate.round
            && existing.certificate.cert_type == certificate.cert_type
    });

    if already_known {
        debug!(
            %certificate.height,
            %certificate.round,
            "Round certificate already known, ignoring"
        );

        let votes = certificate.votes(&state.ctx).collect();
        record_certificate_equivocations(co, state, votes).await?;

        return Ok(());
    }

    let votes: Vec<SignedVote<Ctx>> = certificate.votes(&state.ctx).collect();

    // Skip verification and WAL appends when every vote is already absorbed by
    // the keeper — a resend, a quorum already collected over gossip, a conflict
    // whose evidence is already stored, or a conflict past the evidence cap. An
    // empty certificate still goes through verification so it is rejected.
    if !votes.is_empty()
        && votes
            .iter()
            .all(|vote| round_certificate_vote_absorbed(state, vote))
    {
        debug!(
            %certificate.height,
            %certificate.round,
            "Round certificate votes already known, ignoring"
        );
        return Ok(());
    }

    let validator_set = state.validator_set();

    let validity = verify_round_certificate(
        co,
        certificate.clone(),
        validator_set.clone(),
        state.params.threshold_params,
    )
    .await?;

    if let Err(e) = validity {
        warn!(?certificate, "Invalid round certificate: {e}");
        return Ok(());
    }

    // For round certificates, we process votes one by one, unlike polka and commit certificates,
    // which we process as a whole. The reason for this difference lies in how driver handles equivocated votes.
    //
    // If we were to process polka or commit certificates vote by vote, any equivocated vote (i.e. a vote
    // that conflicts with an already received vote from the same validator) would be discarded. This would
    // cause us to ignore equivocated votes that are part of the certificate and which are important for
    // correct operation of the protocol. To avoid this, we process polka and commit certificates as a whole.
    //
    // For round certificates, however, this is not necessary. It suffices that at least one valid vote
    // (either from the certificate or already present in the system) is processed. Thus, discarding an
    // equivocated vote from the round certificate does not affect correctness.
    //
    // As a result, we decided to simplify the logic for round certificates by handling their votes individually.
    // This avoids extra complexity and edge case handling in the driver.
    //
    // We persist each vote to the WAL the same way a vote received over the network is persisted, rather
    // than the certificate as a whole. On restart the vote keeper re-aggregates the replayed votes and
    // reconstructs the round certificate through the ordinary vote path — no dedicated WAL entry needed.
    //
    // The certificate's round is not bounded relative to the round replay starts from, so this relies on
    // replay being exempt from the future-round bound applied in `on_vote`.
    let round_on_entry = state.round();
    let mut applied_any_vote = false;

    for vote in votes {
        if round_certificate_vote_absorbed(state, &vote) {
            continue;
        }

        // RoundSignature has no extension field, so a rebuilt precommit never
        // carries one. Apply the same policy as on_vote: under Required,
        // votes without extensions are dropped before WAL append and driver
        // apply, keeping WAL replay consistent with the policy.
        //
        // A Skip or PrecommitAny certificate whose threshold sits only in
        // those dropped votes becomes a no-op under Required. Honest skip
        // still happens through prevotes or nil precommits, which this gate
        // still applies.
        if !verify_vote_extension(co, state, &vote).await? {
            continue;
        }

        perform!(
            co,
            Effect::WalAppend(
                certificate.height,
                Input::Vote(vote.clone()),
                Default::default()
            )
        );

        apply_driver_input(co, state, metrics, DriverInput::Vote(vote)).await?;
        applied_any_vote = true;
    }

    let entered_certificate_round =
        round_on_entry < certificate.round && certificate.round == state.round();

    if !entered_certificate_round && applied_any_vote {
        perform!(
            co,
            Effect::CancelTimeout(Timeout::rebroadcast(certificate.round), Default::default())
        );
    }

    Ok(())
}

fn round_certificate_vote_absorbed<Ctx>(state: &State<Ctx>, vote: &SignedVote<Ctx>) -> bool
where
    Ctx: Context,
{
    let votes = state.driver.votes();
    votes.has_vote(vote)
        || votes.is_saturated_conflict(vote)
        || votes.has_equivocation_evidence(vote)
}
