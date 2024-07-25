use malachite_common::{self as common, NilOrVal, Round, VoteType, VotingPower};
use malachite_starknet_p2p_types::{
    Address, BlockHash, Height, Proposal, ProposalPart, PublicKey, Validator, ValidatorSet, Vote,
};

use crate::mock::context::MockContext;

impl common::ProposalPart<MockContext> for ProposalPart {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn sequence(&self) -> u64 {
        self.sequence
    }

    fn validator_address(&self) -> &Address {
        &self.validator
    }
}

impl common::Proposal<MockContext> for Proposal {
    fn height(&self) -> Height {
        self.height
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &BlockHash {
        &self.block_hash
    }

    fn take_value(self) -> BlockHash {
        self.block_hash
    }

    fn pol_round(&self) -> Round {
        self.pol_round
    }

    fn validator_address(&self) -> &Address {
        &self.proposer
    }
}

impl common::Vote<MockContext> for Vote {
    fn height(&self) -> Height {
        self.block_number
    }

    fn round(&self) -> Round {
        self.round
    }

    fn value(&self) -> &NilOrVal<BlockHash> {
        &self.block_hash
    }

    fn take_value(self) -> NilOrVal<BlockHash> {
        self.block_hash
    }

    fn vote_type(&self) -> VoteType {
        self.vote_type
    }

    fn validator_address(&self) -> &Address {
        &self.voter
    }
}

impl common::ValidatorSet<MockContext> for ValidatorSet {
    fn count(&self) -> usize {
        self.validators.len()
    }

    fn total_voting_power(&self) -> VotingPower {
        self.total_voting_power()
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        self.get_by_address(address)
    }

    fn get_by_index(&self, index: usize) -> Option<&Validator> {
        self.validators.get(index)
    }
}

impl common::Validator<MockContext> for Validator {
    fn address(&self) -> &Address {
        &self.address
    }

    fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    fn voting_power(&self) -> VotingPower {
        self.voting_power
    }
}
