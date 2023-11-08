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
        mc::Round::new(i64::from(self.0.round.value()))
    }

    fn value(&self) -> &Block {
        todo!()
    }

    fn pol_round(&self) -> mc::Round {
        self.0
            .pol_round
            .map_or(mc::Round::Nil, |r| mc::Round::new(i64::from(r.value())))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(tm::block::Id);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block(tm::block::Block);

impl mc::Value for Block {
    type Id = BlockId;

    fn id(&self) -> Self::Id {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator(tm::validator::Info);

impl mc::Validator<Context> for Validator {
    fn address(&self) -> Address {
        Address(self.0.address)
    }

    fn public_key(&self) -> &mc::PublicKey<Context> {
        todo!()
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

    fn get_proposer(&self) -> &Validator {
        todo!()
    }

    fn get_by_public_key(&self, public_key: &mc::PublicKey<Context>) -> Option<&Validator> {
        todo!()
    }

    fn get_by_address(&self, address: &Address) -> Option<&Validator> {
        todo!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote(tm::vote::Vote);

impl mc::Vote<Context> for Vote {
    fn round(&self) -> mc::Round {
        todo!()
    }

    fn value(&self) -> Option<&BlockId> {
        todo!()
    }

    fn take_value(self) -> Option<BlockId> {
        todo!()
    }

    fn vote_type(&self) -> mc::VoteType {
        todo!()
    }

    fn validator_address(&self) -> &Address {
        todo!()
    }
}

impl mc::Context for Context {
    type Address = Address;
    type Height = Height;
    type Proposal = Proposal;
    type Validator = Validator;
    type ValidatorSet = ValidatorSet;
    type Value = Block;
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
        value: Block,
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
