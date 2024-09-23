use malachite_common::{Context, Proposal, Round, SignedProposal, Validity, Value};
use malachite_consensus::ProposedValue;
use std::collections::BTreeMap;
use tracing::{debug, warn};

/// This module is responsible for collecting proposed values and consensus proposal messages for
/// a given (height, round).
/// When a new_value is received from the value builder the following entry is stored:
/// `FullProposal { Some(new_value.value, new_value.validity), None }`
///
/// When a new_proposal is received from consensus gossip the following entry is stored:
/// `FullProposal { None, Some(new_proposal) }`
///
/// When both proposal and values have been received, the entry for (height, round) should be:
/// `FullProposal { Some(value.value, value.validity), Some(proposal) }`
///
/// It is possible that a proposer sends two (builder_value, proposal) pairs for same `(height, round)`.
/// In this case, both are stored and we consider that the proposer is equivocating.
/// Currently, the actual equivocation is caught deeper in the consensus crate, through consensus actor
/// propagating both proposals.
///
/// Note: In the future when we support implicit proposal message:
/// - store_proposal() will never be called
/// - get_full_proposal() should only check the presence of `builder_value`

#[derive(Clone, Debug)]
pub struct FullProposal<Ctx: Context> {
    // Value as recevied from the builder and its validity view
    builder_value: Option<(Ctx::Value, Validity)>,
    proposal: Option<SignedProposal<Ctx>>,
}

impl<Ctx: Context> FullProposal<Ctx> {
    pub fn new(
        builder_value: Option<(Ctx::Value, Validity)>,
        proposal: Option<SignedProposal<Ctx>>,
    ) -> Self {
        Self {
            builder_value,
            proposal,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FullProposalKeeper<Ctx: Context> {
    pub full_proposal_keeper: BTreeMap<(Ctx::Height, Round), Vec<FullProposal<Ctx>>>,
}

impl<Ctx: Context> FullProposalKeeper<Ctx> {
    pub fn new() -> Self {
        Self {
            full_proposal_keeper: BTreeMap::new(),
        }
    }
    pub fn get_full_proposal(
        &self,
        height: &Ctx::Height,
        round: Round,
        value: &Ctx::Value,
    ) -> Option<(SignedProposal<Ctx>, Validity)> {
        let proposals = self.full_proposal_keeper.get(&(*height, round));
        match proposals {
            None => None,
            Some(proposals) if proposals.is_empty() => None,
            Some(proposals) => {
                for p in proposals.iter() {
                    match (p.builder_value.clone(), p.proposal.clone()) {
                        (Some((_, validity)), Some(prop)) => {
                            if prop.value().id() == value.id() {
                                return Some((prop, validity));
                            }
                        }
                        _ => {
                            return None;
                        }
                    }
                }
                None
            }
        }
    }

    pub fn store_proposal(&mut self, new_proposal: SignedProposal<Ctx>) {
        let entry = self
            .full_proposal_keeper
            .get_mut(&(new_proposal.height(), new_proposal.round()));
        match entry {
            None => {
                // First time we see something (a proposal) for this height and round
                // Create a full proposal with just the proposal
                let full_proposal = FullProposal {
                    builder_value: None,
                    proposal: Some(new_proposal.clone()),
                };
                self.full_proposal_keeper.insert(
                    (new_proposal.height(), new_proposal.round()),
                    vec![full_proposal],
                );
            }
            Some(full_proposals) => {
                // We have seen values and/ or proposals for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for p in full_proposals.iter_mut() {
                    let FullProposal {
                        builder_value,
                        proposal: existing_proposal,
                        ..
                    } = p;
                    match (builder_value, existing_proposal) {
                        (Some((value, _)), None) => {
                            if value == new_proposal.value() {
                                // Found a matching value. Add the proposal
                                p.proposal = Some(new_proposal);
                                return;
                            }
                        }
                        (_, Some(proposal)) => {
                            if proposal.value() == new_proposal.value() {
                                // Redundant proposal
                                return;
                            }
                        }
                        (_, _) => {
                            panic!("Should never have empty entries")
                        }
                    }
                }

                // Append new proposal
                full_proposals.push(FullProposal::new(None, Some(new_proposal.clone())));
            }
        }
    }

    pub fn store_value(&mut self, new_value: ProposedValue<Ctx>) {
        let entry = self
            .full_proposal_keeper
            .get_mut(&(new_value.height, new_value.round));
        match entry {
            None => {
                // First time we see something (a proposed value) for this height and round
                // Create a full proposal with just the proposal
                let full_proposal = FullProposal {
                    builder_value: Some((new_value.value, new_value.validity)),
                    proposal: None,
                };
                self.full_proposal_keeper
                    .insert((new_value.height, new_value.round), vec![full_proposal]);
            }
            Some(full_proposals) => {
                // We have seen proposals and/ or values for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                for p in full_proposals.iter_mut() {
                    let FullProposal {
                        builder_value: existing_value,
                        proposal,
                        ..
                    } = p;
                    match (existing_value, proposal) {
                        (None, Some(proposal)) => {
                            if proposal.value().id() == new_value.value.id() {
                                // Found a matching proposal. Change the entry at index i
                                p.builder_value = Some((new_value.value, new_value.validity));
                                return;
                            }
                        }
                        (Some((value, _)), _) => {
                            if value.id() == new_value.value.id() {
                                // Same value received before, nothing to do.
                                return;
                            }
                        }
                        (_, _) => {
                            panic!("Should never have empty entries")
                        }
                    }
                }
                // Append new value
                full_proposals.push(FullProposal::new(
                    Some((new_value.value, new_value.validity)),
                    None,
                ));
            }
        }
    }

    pub fn remove_full_proposals(&mut self, height: Ctx::Height, round: Round) {
        // TODO - keep some heights back?
        debug!("Removing full proposals {} {}", height, round);
        let result = self.full_proposal_keeper.remove_entry(&(height, round));
        match result {
            None => {
                warn!(
                    "Full proposals absent for height {} and round {}",
                    height, round
                );
            }
            Some((_key, removed)) => {
                debug!("Removed {} full proposals", removed.len());
            }
        }
    }
}

impl<Ctx: Context> Default for FullProposalKeeper<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}
