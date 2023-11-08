#![allow(unused_variables)]

use malachite_common as mc;
use tendermint as tm;

pub mod ed25519;

#[derive(Copy, Clone, Debug)]
pub struct Context;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address(tm::account::Id);
impl mc::Address for Address {}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Height(tm::block::Height);
impl mc::Height for Height {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal(tm::proposal::Proposal);

impl mc::Proposal<Context> for Proposal {
    fn height(&self) -> Height {
        Height(self.0.height)
    }

    fn round(&self) -> mc::Round {
        mc::Round::from(self.0.round.value())
    }

    fn value(&self) -> BlockId {
        BlockId(self.0.block_id.unwrap()) // FIXME: unwrap
    }

    fn pol_round(&self) -> mc::Round {
        mc::Round::from(self.0.pol_round.map(|r| r.value()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(tm::block::Id);

impl mc::Value for BlockId {
    type Id = BlockId;

    fn id(&self) -> BlockId {
        self.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator(tm::validator::Info);

impl mc::Validator<Context> for Validator {
    fn address(&self) -> Address {
        Address(self.0.address)
    }

    fn public_key(&self) -> mc::PublicKey<Context> {
        match self.0.pub_key {
            tm::PublicKey::Ed25519(key) => {
                // FIXME: unwrap
                let vk = ed25519_consensus::VerificationKey::try_from(key).unwrap();
                ed25519::PublicKey::new(vk)
            }
            _ => unreachable!(), // FIXME: unreachable
        }
    }

    fn voting_power(&self) -> mc::VotingPower {
        self.0.power.value()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorSet(tm::validator::Set);

impl mc::ValidatorSet<Context> for ValidatorSet {
    fn total_voting_power(&self) -> mc::VotingPower {
        self.0.total_voting_power().value()
    }

    fn get_by_address(&self, address: &Address) -> Option<Validator> {
        self.0.validator(address.0).map(Validator)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote(tm::vote::Vote);

impl mc::Vote<Context> for Vote {
    fn round(&self) -> mc::Round {
        mc::Round::from(self.0.round.value())
    }

    fn value(&self) -> Option<BlockId> {
        self.0.block_id.as_ref().map(|&id| BlockId(id))
    }

    fn vote_type(&self) -> mc::VoteType {
        match self.0.vote_type {
            tm::vote::Type::Prevote => mc::VoteType::Prevote,
            tm::vote::Type::Precommit => mc::VoteType::Precommit,
        }
    }

    fn validator_address(&self) -> Address {
        Address(self.0.validator_address)
    }
}

impl mc::Context for Context {
    type Address = Address;
    type Height = Height;
    type Proposal = Proposal;
    type Validator = Validator;
    type ValidatorSet = ValidatorSet;
    type Value = BlockId;
    type Vote = Vote;
    type SigningScheme = crate::ed25519::Ed25519;

    fn sign_vote(vote: &Self::Vote, private_key: &mc::PrivateKey<Self>) -> mc::Signature<Self> {
        todo!()
    }

    fn verify_signed_vote(
        signed_vote: &mc::SignedVote<Self>,
        public_key: &mc::PublicKey<Self>,
    ) -> bool {
        todo!()
    }

    fn new_proposal(
        height: Height,
        round: mc::Round,
        value: BlockId,
        pol_round: mc::Round,
    ) -> Proposal {
        todo!()
    }

    fn new_prevote(round: mc::Round, value_id: Option<BlockId>, address: Address) -> Self::Vote {
        todo!()
    }

    fn new_precommit(round: mc::Round, value_id: Option<BlockId>, address: Address) -> Self::Vote {
        todo!()
    }
}
