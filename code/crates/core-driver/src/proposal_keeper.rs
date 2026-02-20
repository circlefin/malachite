//! For storing proposals.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use derive_where::derive_where;
use thiserror::Error;

use malachitebft_core_types::{
    Context, DoubleProposal, Proposal, Round, SignedProposal, Validity, Value, ValueId,
};
use tracing::{error, warn};

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
}

/// The proposals received in a given round, if any.
#[derive_where(Clone, Debug, PartialEq, Eq, Default)]
pub struct PerRound<Ctx>
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
    pub fn get_proposals_and_validities(&self) -> &[(SignedProposal<Ctx>, Validity)] {
        &self.proposals
    }

    /// Add a proposal to this round, checking for conflicts.
    ///
    /// All proposals must come from the same validator (proposer).
    /// If a proposal comes from a different validator than the first,
    /// this is considered a calling code bug and the function will panic.
    ///
    /// - Stores each unique proposal once.
    /// - Returns an error if equivocation is detected from the **same** validator.
    /// - Panics if proposals come from **different validators**.
    pub fn add(
        &mut self,
        proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Result<(), RecordProposalError<Ctx>> {
        // Early return for exact duplicates
        if self.contains_exact(&proposal, validity) {
            return Ok(());
        }

        // Ensure all proposals come from the same validator
        self.verify_same_validator(&proposal);

        // Update existing proposal or add new one
        match self.proposal_validity_mut(&proposal) {
            Some(existing_validity) => {
                Self::update_validity(&proposal, existing_validity, validity);
            }
            None => {
                self.proposals.push((proposal.clone(), validity));
            }
        }

        // Check for equivocation (multiple distinct proposals)
        self.check_equivocation(proposal)
    }

    fn contains_exact(&self, proposal: &SignedProposal<Ctx>, validity: Validity) -> bool {
        self.proposals
            .iter()
            .any(|(p, v)| p == proposal && *v == validity)
    }

    fn verify_same_validator(&self, proposal: &SignedProposal<Ctx>) {
        if let Some(first) = self.get_first_proposal() {
            assert_eq!(
                first.validator_address(),
                proposal.validator_address(),
                "BUG: Received proposals from different validators in the same round.\n\
                Existing: {:?}, New: {:?}",
                first.validator_address(),
                proposal.validator_address()
            );
        }
    }

    fn proposal_validity_mut(&mut self, proposal: &SignedProposal<Ctx>) -> Option<&mut Validity> {
        self.proposals
            .iter_mut()
            .find(|(p, _)| p == proposal)
            .map(|(_, v)| v)
    }

    fn update_validity(proposal: &SignedProposal<Ctx>, current: &mut Validity, new: Validity) {
        use Validity::{Invalid, Valid};

        match (&current, &new) {
            (Invalid, Valid) => {
                warn!(
                    height = %proposal.message.height(),
                    round = %proposal.message.round(),
                    value_id = %proposal.message.value().id(),
                    "Application changed its mind on proposal's validity: Invalid --> Valid"
                );
                *current = new;
            }
            (Valid, Invalid) => {
                error!(
                    height = %proposal.message.height(),
                    round = %proposal.message.round(),
                    value_id = %proposal.message.value().id(),
                    "Application changed its mind on proposal's validity: Valid --> Invalid; \
                    this should not happen"
                );
            }
            _ => {
                // Same validity, no action needed
            }
        }
    }

    fn check_equivocation(
        &self,
        proposal: SignedProposal<Ctx>,
    ) -> Result<(), RecordProposalError<Ctx>> {
        if self.proposals.len() > 1 {
            let existing = self
                .get_first_proposal()
                .expect("at least one proposal should exist")
                .clone();

            Err(RecordProposalError::ConflictingProposal {
                existing,
                conflicting: proposal,
            })
        } else {
            Ok(())
        }
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

    /// Returns the proposal and its validity for the round, matching the value_id, if any.
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
    ) -> &[(SignedProposal<Ctx>, Validity)] {
        self.per_round
            .get(&round)
            .map(PerRound::get_proposals_and_validities)
            .unwrap_or(&[])
    }

    /// Returns all proposals and their validities for all rounds.
    pub fn all_rounds(&self) -> &BTreeMap<Round, PerRound<Ctx>> {
        &self.per_round
    }

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Remove and return all recorded evidence.
    pub fn take_evidence(&mut self) -> EvidenceMap<Ctx> {
        core::mem::take(&mut self.evidence)
    }

    /// Store a proposal, checking for conflicts and storing evidence of equivocation if necessary.
    pub fn store_proposal(&mut self, proposal: SignedProposal<Ctx>, validity: Validity) {
        let per_round = self.per_round.entry(proposal.round()).or_default();

        match per_round.add(proposal, validity) {
            Ok(()) => (),

            Err(RecordProposalError::ConflictingProposal {
                existing,
                conflicting,
            }) => {
                // This is an equivocating proposal
                warn!(
                    "Received equivocating proposal {:?}, existing {:?}",
                    conflicting, existing
                );
                self.evidence.add(existing, conflicting);
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
    map: BTreeMap<Ctx::Address, Vec<DoubleProposal<Ctx>>>,
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
    pub fn get(&self, address: &Ctx::Address) -> Option<&Vec<DoubleProposal<Ctx>>> {
        self.map.get(address)
    }

    /// Add evidence of equivocating proposals, ie. two proposals submitted by the same validator,
    /// but with different values but for the same height and round.
    /// If evidence for the same pair of proposals already exists, it will not be added again.
    ///
    /// # Precondition
    /// - Both proposals must be from the same validator (debug-asserted).
    pub fn add(&mut self, existing: SignedProposal<Ctx>, conflicting: SignedProposal<Ctx>) {
        debug_assert_eq!(
            existing.validator_address(),
            conflicting.validator_address()
        );

        if let Some(evidence) = self.map.get_mut(conflicting.validator_address()) {
            // Check if this evidence already exists (in either order)
            let already_exists = evidence.iter().any(|(e, c)| {
                (e == &existing && c == &conflicting) || (e == &conflicting && c == &existing)
            });
            if !already_exists {
                evidence.push((existing, conflicting));
            }
        } else {
            self.map.insert(
                conflicting.validator_address().clone(),
                vec![(existing, conflicting)],
            );
        }
    }

    /// Iterate over all addresses with recorded proposal equivocations.
    pub fn iter(
        &self,
    ) -> alloc::collections::btree_map::Keys<'_, Ctx::Address, Vec<DoubleProposal<Ctx>>> {
        self.map.keys()
    }
}
