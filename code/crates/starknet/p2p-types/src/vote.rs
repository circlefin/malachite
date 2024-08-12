use malachite_common::{NilOrVal, Round, VoteType};
use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;

use crate::{Address, BlockHash, Height};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vote {
    pub vote_type: VoteType,
    pub block_number: Height,
    pub fork_id: u64,
    pub round: Round,
    pub block_hash: NilOrVal<BlockHash>,
    pub voter: Address,
}

impl Vote {
    pub fn new_prevote(
        block_number: Height,
        round: Round,
        fork_id: u64,
        block_hash: NilOrVal<BlockHash>,
        voter: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Prevote,
            block_number,
            round,
            fork_id,
            block_hash,
            voter,
        }
    }

    pub fn new_precommit(
        height: Height,
        round: Round,
        fork_id: u64,
        value: NilOrVal<BlockHash>,
        address: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Precommit,
            block_number: height,
            round,
            fork_id,
            block_hash: value,
            voter: address,
        }
    }

    pub fn to_sign_bytes(&self) -> Vec<u8> {
        malachite_proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl proto::Protobuf for Vote {
    type Proto = p2p_proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self {
            vote_type: proto_to_common_vote_type(proto.vote_type()),
            block_number: Height::new(proto.block_number),
            round: Round::new(i64::from(proto.round)),
            fork_id: proto.fork_id,
            block_hash: match proto.block_hash {
                Some(block_hash) => NilOrVal::Val(BlockHash::from_proto(block_hash)?),
                None => NilOrVal::Nil,
            },
            voter: Address::from_proto(
                proto
                    .voter
                    .ok_or_else(|| proto::Error::missing_field::<Self::Proto>("voter"))?,
            )?,
        })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(Self::Proto {
            vote_type: common_to_proto_vote_type(self.vote_type).into(),
            block_number: self.block_number.as_u64(),
            round: self.round.as_i64() as u32, // FIXME: This is a hack
            fork_id: self.fork_id,
            block_hash: match &self.block_hash {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.to_proto()?),
            },
            voter: Some(self.voter.to_proto()?),
        })
    }
}

fn common_to_proto_vote_type(vote_type: VoteType) -> malachite_starknet_p2p_proto::vote::VoteType {
    match vote_type {
        VoteType::Prevote => p2p_proto::vote::VoteType::Prevote,
        VoteType::Precommit => p2p_proto::vote::VoteType::Precommit,
    }
}

fn proto_to_common_vote_type(vote_type: p2p_proto::vote::VoteType) -> VoteType {
    match vote_type {
        p2p_proto::vote::VoteType::Prevote => VoteType::Prevote,
        p2p_proto::vote::VoteType::Precommit => VoteType::Precommit,
    }
}
