//! For storing proposals.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::{Context, Proposal, Round, SignedProposal, Validity};
use tracing::warn;

/// Errors can that be yielded when recording a proposal.
#[derive_where(Debug)]
#[derive(Error)]
pub enum RecordProposalError<Ctx>
where
    Ctx: Context,
{
    /// Attempted to record a conflicting proposal.
    #[error("Conflicting proposal: existing: {existing}, conflicting: {conflicting}")]
    ConflictingProposal {
        /// The proposal already recorded for the same value.
        existing: SignedProposal<Ctx>,
        /// The conflicting proposal, from the same validator.
        conflicting: SignedProposal<Ctx>,
    },

    /// Attempted to record a conflicting proposal from a different validator.
    #[error("Invalid conflicting proposal: existing: {existing}, conflicting: {conflicting}")]
    InvalidConflictingProposal {
        /// The proposal already recorded for the same value.
        existing: SignedProposal<Ctx>,
        /// The conflicting proposal, from a different validator.
        conflicting: SignedProposal<Ctx>,
    },
}

#[derive_where(Clone, Debug, PartialEq, Eq, Default)]
struct PerRound<Ctx>
where
    Ctx: Context,
{
    /// The proposal received in a given round (proposal.round) if any.
    proposal: Option<(SignedProposal<Ctx>, Validity)>,
}

impl<Ctx> PerRound<Ctx>
where
    Ctx: Context,
{
    /// Add a proposal to the round, checking for conflicts.
    pub fn add(
        &mut self,
        proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Result<(), RecordProposalError<Ctx>> {
        if let Some((existing, _)) = self.get_proposal() {
            if existing.value() != proposal.value() {
                if existing.validator_address() != proposal.validator_address() {
                    // This is not a valid equivocating proposal, since the two proposers are different
                    // We should never reach this point, since the consensus algorithm should prevent this.
                    return Err(RecordProposalError::InvalidConflictingProposal {
                        existing: existing.clone(),
                        conflicting: proposal,
                    });
                }

                // This is an equivocating proposal
                return Err(RecordProposalError::ConflictingProposal {
                    existing: existing.clone(),
                    conflicting: proposal,
                });
            }
        }

        // Add the proposal
        self.proposal = Some((proposal, validity));

        Ok(())
    }

    /// Return the proposal received from the given validator.
    pub fn get_proposal(&self) -> Option<&(SignedProposal<Ctx>, Validity)> {
        self.proposal.as_ref()
    }
}

/// Keeps track of proposals.
#[derive_where(Clone, Debug, Default)]
pub struct ProposalKeeper<Ctx>
where
    Ctx: Context,
{
    /// The proposal for each round.
    per_round: BTreeMap<Round, PerRound<Ctx>>,

    /// Evidence of equivocation.
    evidence: EvidenceMap<Ctx>,
}

impl<Ctx> ProposalKeeper<Ctx>
where
    Ctx: Context,
{
    /// Create a new `ProposalKeeper` instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the proposal and validity for the round.
    pub fn get_proposal_and_validity_for_round(
        &self,
        round: Round,
    ) -> Option<&(SignedProposal<Ctx>, Validity)> {
        self.per_round
            .get(&round)
            .and_then(|round_info| round_info.proposal.as_ref())
    }

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Store a proposal, checking for conflicts and storing evidence of equivocation if necessary.
    ///
    /// # Precondition
    /// - The given proposal must have been proposed by the expected proposer at the proposal's height and round.
    pub fn store_proposal(
        &mut self,
        proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Result<(), RecordProposalError<Ctx>> {
        let per_round = self.per_round.entry(proposal.round()).or_default();

        match per_round.add(proposal, validity) {
            Ok(()) => Ok(()),

            Err(RecordProposalError::ConflictingProposal {
                existing,
                conflicting,
            }) => {
                // This is an equivocating proposal
                self.evidence.add(existing.clone(), conflicting.clone());

                warn!(
                    "Conflicting proposal: existing: {:?}, conflicting: {:?}",
                    existing, conflicting
                );

                Err(RecordProposalError::ConflictingProposal {
                    existing,
                    conflicting,
                })
            }

            Err(RecordProposalError::InvalidConflictingProposal {
                existing,
                conflicting,
            }) => {
                // This is not a valid equivocating proposal, since the two proposers are different
                // We should never reach this point, since the consensus algorithm should prevent this.
                unreachable!(
                    "Conflicting proposals from different validators: existing: {}, conflicting: {}",
                    existing.validator_address(), conflicting.validator_address()
                );
            }
        }
    }
}

/// Keeps track of evidence of equivocation.
#[derive_where(Clone, Debug, Default)]
pub struct EvidenceMap<Ctx>
where
    Ctx: Context,
{
    #[allow(clippy::type_complexity)]
    map: BTreeMap<Ctx::Address, Vec<(SignedProposal<Ctx>, SignedProposal<Ctx>)>>,
}

impl<Ctx> EvidenceMap<Ctx>
where
    Ctx: Context,
{
    /// Create a new `EvidenceMap` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether or not there is any evidence of equivocation.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Return the evidence of equivocation for a given address, if any.
    pub fn get(
        &self,
        address: &Ctx::Address,
    ) -> Option<&Vec<(SignedProposal<Ctx>, SignedProposal<Ctx>)>> {
        self.map.get(address)
    }

    /// Add evidence of equivocating proposals, ie. two proposals submitted by the same validator,
    /// but with different values but for the same height and round.
    ///
    /// # Precondition
    /// - Panics if the two conflicting proposals were not proposed by the same validator.
    pub(crate) fn add(&mut self, existing: SignedProposal<Ctx>, conflicting: SignedProposal<Ctx>) {
        assert_eq!(
            existing.validator_address(),
            conflicting.validator_address()
        );

        if let Some(evidence) = self.map.get_mut(conflicting.validator_address()) {
            evidence.push((existing, conflicting));
        } else {
            self.map.insert(
                conflicting.validator_address().clone(),
                vec![(existing, conflicting)],
            );
        }
    }
}
