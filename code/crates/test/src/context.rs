use std::sync::Arc;

use malachite_core_types::{
    Context, NilOrVal, Round, SignedProposal, SignedProposalPart, SignedVote, ValidatorSet as _,
};

use crate::address::*;
use crate::height::*;
use crate::proposal::*;
use crate::proposal_part::*;
use crate::signing::*;
use crate::validator_set::*;
use crate::value::*;
use crate::vote::*;

#[derive(Clone, Debug)]
pub struct TestContext {
    private_key: Arc<PrivateKey>,
}

impl TestContext {
    pub fn new(private_key: PrivateKey) -> Self {
        Self {
            private_key: Arc::new(private_key),
        }
    }
}

impl Context for TestContext {
    type Address = Address;
    type ProposalPart = ProposalPart;
    type Height = Height;
    type Proposal = Proposal;
    type ValidatorSet = ValidatorSet;
    type Validator = Validator;
    type Value = Value;
    type Vote = Vote;
    type SigningScheme = Ed25519;

    fn select_proposer<'a>(
        &self,
        validator_set: &'a Self::ValidatorSet,
        height: Self::Height,
        round: Round,
    ) -> &'a Self::Validator {
        assert!(validator_set.count() > 0);
        assert!(round != Round::Nil && round.as_i64() >= 0);

        let proposer_index = {
            let height = height.as_u64() as usize;
            let round = round.as_i64() as usize;

            (height - 1 + round) % validator_set.count()
        };

        validator_set
            .get_by_index(proposer_index)
            .expect("proposer_index is valid")
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_vote(&self, vote: Self::Vote) -> SignedVote<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&vote.to_bytes());
        SignedVote::new(vote, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_vote(
        &self,
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        use signature::Verifier;
        public_key.verify(&vote.to_bytes(), signature).is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_proposal(&self, proposal: Self::Proposal) -> SignedProposal<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&proposal.to_bytes());
        SignedProposal::new(proposal, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        use signature::Verifier;
        public_key.verify(&proposal.to_bytes(), signature).is_ok()
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn sign_proposal_part(&self, proposal_part: Self::ProposalPart) -> SignedProposalPart<Self> {
        use signature::Signer;
        let signature = self.private_key.sign(&proposal_part.to_bytes());
        SignedProposalPart::new(proposal_part, signature)
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn verify_signed_proposal_part(
        &self,
        proposal_part: &ProposalPart,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        use signature::Verifier;
        public_key
            .verify(&proposal_part.to_bytes(), signature)
            .is_ok()
    }

    fn new_proposal(
        height: Height,
        round: Round,
        value: Value,
        pol_round: Round,
        address: Address,
    ) -> Proposal {
        Proposal::new(height, round, value, pol_round, address)
    }

    fn new_prevote(
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_prevote(height, round, value_id, address)
    }

    fn new_precommit(
        height: Height,
        round: Round,
        value_id: NilOrVal<ValueId>,
        address: Address,
    ) -> Vote {
        Vote::new_precommit(height, round, value_id, address)
    }
}
