use bytes::Bytes;
use starknet_core::utils::starknet_keccak;

use malachitebft_core_types::{
    CertificateError, CommitCertificate, CommitSignature, NilOrVal, SignedExtension,
    SignedProposal, SignedVote, SigningProvider, VotingPower,
};

use crate::{
    MockContext, PrivateKey, Proposal, ProposalPart, PublicKey, Signature, Validator, Vote,
};

#[derive(Debug)]
pub struct EcdsaProvider {
    private_key: PrivateKey,
}

impl EcdsaProvider {
    pub fn new(private_key: PrivateKey) -> Self {
        Self { private_key }
    }
}

impl SigningProvider<MockContext> for EcdsaProvider {
    fn sign_vote(&self, vote: Vote) -> SignedVote<MockContext> {
        // Votes are not signed for now
        // let hash = starknet_keccak(&vote.to_sign_bytes());
        // let signature = self.private_key.sign(&hash);
        SignedVote::new(vote, Signature::dummy())
    }

    fn verify_signed_vote(
        &self,
        _vote: &Vote,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Votes are not signed for now
        true
        // let hash = starknet_keccak(&vote.to_sign_bytes());
        // public_key.verify(&hash, signature)
    }

    fn sign_proposal(&self, proposal: Proposal) -> SignedProposal<MockContext> {
        // Proposals are never sent over the network
        SignedProposal::new(proposal, Signature::dummy())
    }

    fn verify_signed_proposal(
        &self,
        _proposal: &Proposal,
        _signature: &Signature,
        _public_key: &PublicKey,
    ) -> bool {
        // Proposals are never sent over the network
        true
    }

    fn sign_vote_extension(&self, extension: Bytes) -> SignedExtension<MockContext> {
        let hash = starknet_keccak(extension.as_ref());
        let signature = self.private_key.sign(&hash);
        SignedExtension::new(extension, signature)
    }

    fn verify_signed_vote_extension(
        &self,
        extension: &Bytes,
        signature: &Signature,
        public_key: &PublicKey,
    ) -> bool {
        let hash = starknet_keccak(extension.as_ref());
        public_key.verify(&hash, signature)
    }

    fn verify_commit_signature(
        &self,
        certificate: &CommitCertificate<MockContext>,
        commit_sig: &CommitSignature<MockContext>,
        validator: &Validator,
    ) -> Result<VotingPower, CertificateError<MockContext>> {
        use malachitebft_core_types::Validator;

        // Reconstruct the vote that was signed
        let vote = Vote::new_precommit(
            certificate.height,
            certificate.round,
            NilOrVal::Val(certificate.value_id),
            *validator.address(),
        );

        // Verify signature
        if !self.verify_signed_vote(&vote, &commit_sig.signature, validator.public_key()) {
            return Err(CertificateError::InvalidSignature(commit_sig.clone()));
        }

        Ok(validator.voting_power())
    }
}
