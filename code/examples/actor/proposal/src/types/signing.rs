use bytes::Bytes;

use malachitebft_core_types::{
    SignedExtension, SignedProposal, SignedProposalPart, SignedVote, SigningProvider,
};

use crate::types::{
    context::MockContext, proposal::Proposal, proposal_part::ProposalPart, vote::Vote,
};

pub use malachitebft_signing_ed25519::*;

pub trait Hashable {
    type Output;
    fn hash(&self) -> Self::Output;
}

impl Hashable for PublicKey {
    type Output = [u8; 32];

    fn hash(&self) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(self.as_bytes());
        hasher.finalize().into()
    }
}

#[derive(Debug)]
pub struct Ed25519Provider {
    private_key: PrivateKey,
}

impl Ed25519Provider {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }

    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        self.private_key.sign(data)
    }

    pub fn verify(&self, data: &[u8], signature: &Signature, public_key: &PublicKey) -> bool {
        public_key.verify(data, signature).is_ok()
    }
}

impl SigningProvider<MockContext> for Ed25519Provider {
    fn sign_vote(&self, vote: Vote) -> SignedVote<MockContext> {
        let signature = self.sign(&vote.to_sign_bytes());
        SignedVote::new(vote, signature)
    }

    fn verify_signed_vote(
        &self,
        vote: &Vote,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key.verify(&vote.to_sign_bytes(), signature).is_ok()
    }

    fn sign_proposal(&self, proposal: Proposal) -> SignedProposal<MockContext> {
        let signature = self.private_key.sign(&proposal.to_sign_bytes());
        SignedProposal::new(proposal, signature)
    }

    fn verify_signed_proposal(
        &self,
        proposal: &Proposal,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key
            .verify(&proposal.to_sign_bytes(), signature)
            .is_ok()
    }

    fn sign_proposal_part(&self, _proposal_part: ProposalPart) -> SignedProposalPart<MockContext> {
        panic!("Not applicable for ProposalOnly mode");
    }

    fn verify_signed_proposal_part(
        &self,
        _proposal_part: &ProposalPart,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        panic!("Not applicable for ProposalOnly mode");
    }

    fn sign_vote_extension(&self, extension: Bytes) -> SignedExtension<MockContext> {
        let signature = self.private_key.sign(extension.as_ref());
        malachitebft_core_types::SignedMessage::new(extension, signature)
    }

    fn verify_signed_vote_extension(
        &self,
        extension: &Bytes,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        public_key.verify(extension.as_ref(), signature).is_ok()
    }
}
