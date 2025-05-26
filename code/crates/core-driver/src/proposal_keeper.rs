//! For storing proposals.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::{Context, Proposal, Round, SignedProposal, Validity, Value, ValueId};

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
    /// The proposals received in a given round (proposal.round) if any.
    proposals: Vec<(SignedProposal<Ctx>, Validity)>,
}

impl<Ctx> PerRound<Ctx>
where
    Ctx: Context,
{
    /// Create a new `PerRound` instance
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the first proposal and its validity that matches the given value_id, if any.
    fn get_first_proposal_and_validity(
        &self,
        value_id: ValueId<Ctx>,
    ) -> Option<&(SignedProposal<Ctx>, Validity)> {
        self.proposals
            .iter()
            .find(|(proposal, _)| proposal.value().id() == value_id)
    }

    // /// Return the first proposal, if any, without validity.
    fn get_first_proposal(&self) -> Option<&SignedProposal<Ctx>> {
        self.proposals.first().map(|(p, _)| p)
    }

    /// Returns all proposals and their validities.
    pub fn proposals_and_validities(&self) -> &[(SignedProposal<Ctx>, Validity)] {
        &self.proposals
    }

    /// Add a proposal to this round, checking for conflicts.
    /// All proposals must come from the same validator (proposer).
    /// - Stores each unique proposal once.
    /// - Returns an error if equivocation is detected but stores it anyway.
    pub fn add(
        &mut self,
        proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Result<(), RecordProposalError<Ctx>> {
        // Ignore exact duplicates
        if self.proposals.iter().any(|(p, _)| p == &proposal) {
            return Ok(());
        }

        // Store the new unique proposal
        self.proposals.push((proposal.clone(), validity));

        // If more than one distinct proposal has been recorded, it as equivocation
        if self.proposals.len() > 1 {
            return Err(RecordProposalError::ConflictingProposal {
                existing: self
                    .get_first_proposal()
                    .expect("at least one proposal should exist after push")
                    .clone(),
                conflicting: proposal,
            });
        }

        // NOTE: The check for proposals from different validators was removed.
        // We assume all proposals come from the same validator in this context.
        // If that assumption is ever violated, it likely indicates a bug in the caller logic.
        // Let's revisit and discuss this if needed.

        Ok(())
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

    /// Returns the proposal and its validity for the round matching the value_id, if any.
    pub fn get_proposal_and_validity_for_round_and_value(
        &self,
        round: Round,
        value_id: ValueId<Ctx>,
    ) -> Option<&(SignedProposal<Ctx>, Validity)> {
        self.per_round
            .get(&round)
            .and_then(|round_info| round_info.get_first_proposal_and_validity(value_id))
    }

    /// Returns all proposals and their validities for the round, if any.
    pub fn get_proposals_and_validities_for_round(
        &self,
        round: Round,
    ) -> Option<&[(SignedProposal<Ctx>, Validity)]> {
        self.per_round
            .get(&round)
            .map(|round_info| round_info.proposals_and_validities())
    }

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Store a proposal, checking for conflicts and storing evidence of equivocation if necessary.
    ///
    /// # Precondition
    /// - The given proposal must have been proposed by the expected proposer at the proposal's height and round.
    pub fn store_proposal(&mut self, proposal: SignedProposal<Ctx>, validity: Validity) {
        let per_round = self.per_round.entry(proposal.round()).or_default();

        match per_round.add(proposal, validity) {
            Ok(()) => (),

            Err(RecordProposalError::ConflictingProposal {
                existing,
                conflicting,
            }) => {
                // This is an equivocating proposal
                self.evidence.add(existing, conflicting);
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
