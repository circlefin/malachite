use bytes::Bytes;

use malachitebft_core_types::{NilOrVal, Round, VoteType};
use malachitebft_proto as proto;
use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::{Address, BlockHash, Height};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vote {
    pub vote_type: VoteType,
    pub height: Height,
    pub round: Round,
    pub block_hash: NilOrVal<BlockHash>,
    pub voter: Address,
}

impl Vote {
    pub fn new_prevote(
        height: Height,
        round: Round,
        block_hash: NilOrVal<BlockHash>,
        voter: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Prevote,
            height,
            round,
            block_hash,
            voter,
        }
    }

    pub fn new_precommit(
        height: Height,
        round: Round,
        value: NilOrVal<BlockHash>,
        address: Address,
    ) -> Self {
        Self {
            vote_type: VoteType::Precommit,
            height,
            round,
            block_hash: value,
            voter: address,
        }
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        malachitebft_proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl proto::Protobuf for Vote {
    type Proto = p2p_proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let vote_type = proto_to_common_vote_type(proto.vote_type());

        Ok(Self {
            vote_type,
            height: Height::new(proto.block_number, proto.fork_id),
            round: Round::new(proto.round),
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
            block_number: self.height.block_number,
            fork_id: self.height.fork_id,
            round: self.round.as_u32().expect("round should not be nil"),
            block_hash: match &self.block_hash {
                NilOrVal::Nil => None,
                NilOrVal::Val(v) => Some(v.to_proto()?),
            },
            voter: Some(self.voter.to_proto()?),
        })
    }
}

fn common_to_proto_vote_type(vote_type: VoteType) -> malachitebft_starknet_p2p_proto::VoteType {
    match vote_type {
        VoteType::Prevote => p2p_proto::VoteType::Prevote,
        VoteType::Precommit => p2p_proto::VoteType::Precommit,
    }
}

fn proto_to_common_vote_type(vote_type: p2p_proto::VoteType) -> VoteType {
    match vote_type {
        p2p_proto::VoteType::Prevote => VoteType::Prevote,
        p2p_proto::VoteType::Precommit => VoteType::Precommit,
    }
}
