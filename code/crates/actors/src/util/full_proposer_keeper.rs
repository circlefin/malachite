use malachite_common::{Context, Proposal, Round, SignedProposal, Validity, Value};
use malachite_consensus::ProposedValue;
use std::collections::BTreeMap;
use tracing::{debug, warn};

#[derive(Clone, Debug)]
pub struct FullProposal<Ctx: Context> {
    // Value as recevied from the builder and its validity view
    builder_value: Option<(Ctx::Value, Validity)>,
    proposal: Option<SignedProposal<Ctx>>,
    // If builder_value is invalid then invalid, otherwise invalid if builder_value is different than the proposal one
    validity: Option<Validity>,
}

impl<Ctx: Context> FullProposal<Ctx> {
    pub fn new(
        builder_value: Option<(Ctx::Value, Validity)>,
        proposal: Option<SignedProposal<Ctx>>,
        validity: Option<Validity>,
    ) -> Self {
        Self {
            builder_value,
            proposal,
            validity,
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
                    match (p.builder_value.clone(), p.proposal.clone(), p.validity) {
                        (Some(_), Some(prop), Some(validity)) => {
                            if prop.value().id() == value.id() {
                                return Some((prop, validity));
                            } else {
                                continue;
                            }
                        }
                        (Some(_), Some(_), None) => {
                            panic!("null validity when both value and proposal are present");
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
                    validity: None,
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
                let mut append = false;
                let mut some_value_index = 0;
                for (i, p) in full_proposals.iter_mut().enumerate() {
                    let FullProposal {
                        builder_value,
                        proposal: existing_proposal,
                        ..
                    } = p;
                    match (builder_value, existing_proposal) {
                        (Some((value, _)), None) => {
                            if value == new_proposal.value() {
                                // Found a matching value. Change the entry at index i
                                some_value_index = i;
                                break;
                            } else {
                                // Continue to find a matching value
                                continue;
                            }
                        }
                        (None, Some(proposal)) => {
                            if proposal.value() == new_proposal.value() {
                                // Redundant proposal
                                return;
                            } else {
                                // Append equivocating proposal
                                append = true;
                                break;
                            }
                        }
                        (Some((_value, _validity)), Some(proposal)) => {
                            if proposal.value() == new_proposal.value() {
                                // Redundant proposal
                                return;
                            } else {
                                // TODO - figure out what to do here
                            }
                        }
                        (_, _) => {
                            panic!("Should never have empty entries")
                        }
                    }
                }
                if append {
                    // Append new proposal
                    full_proposals.push(FullProposal::new(None, Some(new_proposal.clone()), None));
                    return;
                }
                // Replace proposal at some_value_index
                let mut full_proposal = full_proposals[some_value_index].clone();
                full_proposal.proposal = Some(new_proposal.clone());
                full_proposal.validity =
                    if let Some((ref value, validity)) = full_proposal.builder_value {
                        if value.id() == new_proposal.value().id() {
                            Some(validity)
                        } else {
                            Some(Validity::Invalid)
                        }
                    } else {
                        None
                    };
                full_proposals[some_value_index] = full_proposal.clone();
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
                    validity: Some(new_value.validity),
                };
                self.full_proposal_keeper
                    .insert((new_value.height, new_value.round), vec![full_proposal]);
            }
            Some(full_proposals) => {
                // We have seen proposals and/ or values for this height and round.
                // Iterate over the vector of full proposals and determine if a new entry needs
                // to be appended or an existing one has to be modified.
                let mut append = false;
                let mut some_value_index = 0;
                for (i, p) in full_proposals.iter_mut().enumerate() {
                    let FullProposal {
                        builder_value: existing_value,
                        proposal,
                        ..
                    } = p;
                    match (existing_value, proposal) {
                        (None, Some(proposal)) => {
                            if proposal.value().id() == new_value.value.id() {
                                // Found a matching proposal. Change the entry at index i
                                some_value_index = i;
                                break;
                            } else {
                                // Continue to find a matching value
                                continue;
                            }
                        }
                        (Some((value, _)), None) => {
                            if value.id() == new_value.value.id() {
                                // Same value received before, nothing to do.
                                return;
                            } else {
                                // Append equivocating value
                                append = true;
                                break;
                            }
                        }
                        (Some((value, _)), Some(_)) => {
                            if value.id() == new_value.value.id() {
                                // Same value received before, nothing to do.
                                return;
                            } else {
                                // TODO - figure out what to do here
                            }
                        }
                        (_, _) => {
                            panic!("Should never have empty entries")
                        }
                    }
                }
                if append {
                    // Append new value
                    full_proposals.push(FullProposal::new(
                        Some((new_value.value, new_value.validity)),
                        None,
                        None,
                    ));
                    return;
                }
                // Replace value at some_value_index
                let mut full_proposal = full_proposals[some_value_index].clone();
                full_proposal.validity = if let Some(ref proposal) = full_proposal.proposal {
                    if proposal.value().id() == new_value.value.id() {
                        Some(new_value.validity)
                    } else {
                        Some(Validity::Invalid)
                    }
                } else {
                    None
                };
                full_proposal.builder_value = Some((new_value.value, new_value.validity));
                full_proposals[some_value_index] = full_proposal.clone();
            }
        }
    }

    pub fn remove_proposal(&mut self, height: Ctx::Height, round: Round) {
        // TODO - keep some heights back?
        self.full_proposal_keeper.remove_entry(&(height, round));
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
