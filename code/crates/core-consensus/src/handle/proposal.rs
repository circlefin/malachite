use crate::handle::driver::apply_driver_input;
use crate::handle::signature::verify_signature;
use crate::handle::validator_set::get_validator_set;
use crate::input::Input;
use crate::prelude::*;
use crate::types::{ConsensusMsg, ProposedValue, SignedConsensusMsg, WalEntry};
use crate::util::pretty::PrettyProposal;

/// Handles an incoming consensus proposal message.
///
/// This handler processes proposals that can arrive from three sources:
/// 1. Network messages from other nodes
/// 2. Local proposals when this node is the proposer
/// 3. WAL replay during node restart
///
/// When acting as proposer (2), consensus core interacts with the application to get a proposed value for the current height and round.
/// In this case the proposal message is sent out to the network but also back to the consensus core.
///
/// # Arguments
/// * `co` - The context object containing configuration and external dependencies
/// * `state` - The current consensus state
/// * `metrics` - Metrics collection for monitoring
/// * `signed_proposal` - The signed proposal message to process
///
/// # Flow
/// 1. Validates proposal height and signature
/// 2. Queues messages if not ready to process (wrong height/round)
/// 3. Stores valid proposals and updates WAL if needed
/// 4. Processes the proposal through the driver if a full proposal is available
pub async fn on_proposal<Ctx>(
    co: &Co<Ctx>,
    state: &mut State<Ctx>,
    metrics: &Metrics,
    signed_proposal: SignedProposal<Ctx>,
) -> Result<(), Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.driver.height();

    let proposal_height = signed_proposal.height();
    let proposal_round = signed_proposal.round();
    let proposer_address = signed_proposal.validator_address();

    if proposal_height < consensus_height {
        warn!(
            consensus.height = %consensus_height,
            proposal.height = %proposal_height,
            proposer = %proposer_address,
            "Received proposal for lower height, dropping"
        );

        return Ok(());
    }

    // Queue messages if driver is not initialized, or if they are for higher height.
    // Process messages received for the current height.
    // Drop all others.
    if state.driver.round() == Round::Nil {
        debug!("Received proposal at round -1, queuing for later");
        state.buffer_input(proposal_height, Input::Proposal(signed_proposal));

        return Ok(());
    }

    if proposal_height > consensus_height {
        debug!("Received proposal for higher height {proposal_height}, queuing for later",);
        state.buffer_input(proposal_height, Input::Proposal(signed_proposal));

        return Ok(());
    }

    debug_assert_eq!(proposal_height, consensus_height);

    if !verify_signed_proposal(co, state, &signed_proposal).await? {
        return Ok(());
    }

    info!(
        consensus.height = %consensus_height,
        proposal.height = %proposal_height,
        proposal.round = %proposal_round,
        proposer = %proposer_address,
        message = %PrettyProposal::<Ctx>(&signed_proposal.message),
        "Received proposal"
    );

    // Store the proposal in the full proposal keeper
    state.store_proposal(signed_proposal.clone());

    // If consensus runs in a mode where it publishes proposals over the network,
    // we need to persist in the Write-Ahead Log before we actually send it over the network.
    if state.params.value_payload.include_proposal() {
        perform!(
            co,
            Effect::WalAppend(
                WalEntry::ConsensusMsg(SignedConsensusMsg::Proposal(signed_proposal.clone())),
                Default::default()
            )
        );
    }

    if state.params.value_payload.proposal_only() {
        // TODO - pass the received value up to the host that will verify and give back validity and extension.
        // Currently starknet Context defines value as BlockHash, we need a PoC app for this.
        let new_value = ProposedValue {
            height: signed_proposal.height(),
            round: signed_proposal.round(),
            valid_round: signed_proposal.pol_round(),
            proposer: signed_proposal.validator_address().clone(),
            value: signed_proposal.value().clone(),
            validity: Validity::Valid,
        };

        state.store_value(&new_value);
    }

    if let Some(full_proposal) = state.full_proposal_at_round_and_value(
        &proposal_height,
        proposal_round,
        signed_proposal.value(),
    ) {
        apply_driver_input(
            co,
            state,
            metrics,
            DriverInput::Proposal(full_proposal.proposal.clone(), full_proposal.validity),
        )
        .await?;
    } else {
        debug!(
            proposal.height = %proposal_height,
            proposal.round = %proposal_round,
            "No full proposal for this round yet, stored proposal for later"
        );
    }

    Ok(())
}

pub async fn verify_signed_proposal<Ctx>(
    co: &Co<Ctx>,
    state: &State<Ctx>,
    signed_proposal: &SignedProposal<Ctx>,
) -> Result<bool, Error<Ctx>>
where
    Ctx: Context,
{
    let consensus_height = state.driver.height();
    let proposal_height = signed_proposal.height();
    let proposal_round = signed_proposal.round();
    let proposer_address = signed_proposal.validator_address();

    let Some(validator_set) = get_validator_set(co, state, proposal_height).await? else {
        debug!(
            consensus.height = %consensus_height,
            proposal.height = %proposal_height,
            proposer = %proposer_address,
            "Received proposal for height without known validator set, dropping"
        );

        return Ok(false);
    };

    let Some(proposer) = validator_set.get_by_address(proposer_address) else {
        warn!(
            consensus.height = %consensus_height,
            proposal.height = %proposal_height,
            proposer = %proposer_address,
            "Received proposal from unknown validator"
        );

        return Ok(false);
    };

    let expected_proposer = state.get_proposer(proposal_height, proposal_round);

    if expected_proposer != proposer_address {
        warn!(
            consensus.height = %consensus_height,
            proposal.height = %proposal_height,
            proposer = %proposer_address,
            expected = %expected_proposer,
            "Received proposal from a non-proposer"
        );

        return Ok(false);
    };

    let signed_msg = signed_proposal.clone().map(ConsensusMsg::Proposal);
    if !verify_signature(co, signed_msg, proposer).await? {
        warn!(
            consensus.height = %consensus_height,
            proposal.height = %proposal_height,
            proposer = %proposer_address,
            "Received invalid signature for proposal: {}",
            PrettyProposal::<Ctx>(&signed_proposal.message)
        );

        return Ok(false);
    }

    Ok(true)
}
