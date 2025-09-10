use crate::handle::driver::apply_driver_input;
use crate::handle::validator_set::get_validator_set;
use crate::prelude::*;

use super::signature::{verify_polka_certificate, verify_round_certificate};

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

    let validator_set = get_validator_set(co, state, certificate.height)
        .await?
        .ok_or_else(|| Error::ValidatorSetNotFound(certificate.height))?;

    let validity = verify_polka_certificate(
        co,
        certificate.clone(),
        validator_set.into_owned(),
        state.params.threshold_params,
    )
    .await?;

    if let Err(e) = validity {
        warn!(?certificate, "Invalid polka certificate: {e}");
        return Ok(());
    }

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
/// 1. Validating that the certificate's height matches the current state height
/// 2. Processing each vote signature in the round certificate
/// 3. Converting signatures into appropriate vote types (Prevote or Precommit)
/// 4. Verifying each vote's validity
/// 5. Applying valid votes to the consensus state
///
/// Note: The round certificate can be of type `2f+1` PrecommitAny or `f+1` SkipRound (*).
/// For round certificates, in contrast to polka certificates, the votes are applied
/// individually to the driver and once the threshold is reached it is sent to the state machine.
/// Presence of equivocating votes is not a problem, as the driver will ignore them while
/// the vote keeper will still be able to generate the threshold output using the existing
/// stored and incoming votes from the certificate.
///
/// (*) There is currently no validation of the correctness of the certificate in this function.
/// A byzantine validator may send a certificate that does not have enough votes to reach the
/// thresholds for `PrecommitAny` or `SkipRound`.
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
        warn!(
            %certificate.height,
            consensus.height = %state.height(),
            "Round certificate height mismatch"
        );

        return Ok(());
    }

    match certificate.cert_type {
        RoundCertificateType::Precommit => {
            if certificate.round < state.round() {
                warn!(
                    %certificate.round,
                    consensus.round = %state.round(),
                    "Precommit round certificate from older round"
                );
                return Ok(());
            }
        }
        RoundCertificateType::Skip => {
            if certificate.round <= state.round() {
                warn!(
                    %certificate.round,
                    consensus.round = %state.round(),
                    "Skip round certificate from same or older round"
                );
                return Ok(());
            }
        }
    }

    let validator_set = get_validator_set(co, state, certificate.height)
        .await?
        .ok_or_else(|| Error::ValidatorSetNotFound(certificate.height))?;

    let validity = verify_round_certificate(
        co,
        certificate.clone(),
        validator_set.into_owned(),
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
    for signature in certificate.round_signatures {
        let vote = {
            let vote_msg = match signature.vote_type {
                VoteType::Prevote => state.ctx.new_prevote(
                    certificate.height,
                    certificate.round,
                    signature.value_id,
                    signature.address,
                ),
                VoteType::Precommit => state.ctx.new_precommit(
                    certificate.height,
                    certificate.round,
                    signature.value_id,
                    signature.address,
                ),
            };
            SignedVote::new(vote_msg, signature.signature)
        };

        apply_driver_input(co, state, metrics, DriverInput::Vote(vote)).await?;
    }

    perform!(
        co,
        Effect::CancelTimeout(Timeout::rebroadcast(certificate.round), Default::default())
    );

    Ok(())
}
