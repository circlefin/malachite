//! For storing proposals.

use derive_where::derive_where;

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;

use malachite_common::{Context, Proposal, Round, SignedProposal, Validity};

/// Errors can that be yielded when recording a proposal.
pub enum RecordProposalError<Ctx>
where
    Ctx: Context,
{
    /// Attempted to record a conflicting vote.
    ConflictingProposal {
        /// The proposal already recorded.
        existing: SignedProposal<Ctx>,
        /// The conflicting proposal.
        conflicting: SignedProposal<Ctx>,
    },
}

#[derive_where(Clone, Debug, PartialEq, Eq, Default)]
pub struct PerRound<Ctx>
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
#[derive_where(Clone, Debug)]
pub struct ProposalKeeper<Ctx>
where
    Ctx: Context,
{
    /// The validator set for this height.
    validator_set: Ctx::ValidatorSet,

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
    pub fn new(validator_set: Ctx::ValidatorSet) -> Self {
        Self {
            validator_set,
            per_round: BTreeMap::new(),
            evidence: EvidenceMap::new(),
        }
    }

    /// Return the current validator set
    pub fn validator_set(&self) -> &Ctx::ValidatorSet {
        &self.validator_set
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

    /// Return the proposal and validity for the round.
    pub fn get_proposal_for_round(&self, round: Round) -> Option<&SignedProposal<Ctx>> {
        match self.get_proposal_and_validity_for_round(round) {
            Some((proposal, _)) => Some(proposal),
            None => None,
        }
    }

    /// Return the evidence of equivocation.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        &self.evidence
    }

    /// Apply a proposal.
    pub fn apply_proposal(&mut self, proposal: SignedProposal<Ctx>, validity: Validity) {
        let per_round = self.per_round.entry(proposal.round()).or_default();

        match per_round.add(proposal, validity) {
            Ok(()) => (),
            Err(RecordProposalError::ConflictingProposal {
                existing,
                conflicting,
            }) => {
                // This is an equivocating proposal
                self.evidence.add(existing, conflicting)
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

    /// Add evidence of equivocation.
    pub fn add(&mut self, existing: SignedProposal<Ctx>, proposal: SignedProposal<Ctx>) {
        debug_assert_eq!(existing.validator_address(), proposal.validator_address());

        if let Some(evidence) = self.map.get_mut(proposal.validator_address()) {
            evidence.push((existing, proposal));
        } else {
            self.map.insert(
                proposal.validator_address().clone(),
                vec![(existing, proposal)],
            );
        }
    }
}
