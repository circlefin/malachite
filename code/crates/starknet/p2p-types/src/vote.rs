use bytes::Bytes;

use malachite_common::{Extension, NilOrVal, Round, SignedExtension, VoteType};
use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;

use crate::{Address, BlockHash, Height, MockContext, Signature};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Vote {
    pub vote_type: VoteType,
    pub height: Height,
    pub round: Round,
    pub block_hash: NilOrVal<BlockHash>,
    pub voter: Address,
    pub extension: Option<SignedExtension<MockContext>>,
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
            extension: None,
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
            extension: None,
        }
    }

    pub fn new_precommit_with_extension(
        height: Height,
        round: Round,
        value: NilOrVal<BlockHash>,
        address: Address,
        extension: SignedExtension<MockContext>,
    ) -> Self {
        Self {
            vote_type: VoteType::Precommit,
            height,
            round,
            block_hash: value,
            voter: address,
            extension: Some(extension),
        }
    }

    pub fn to_sign_bytes(&self) -> Bytes {
        malachite_proto::Protobuf::to_bytes(self).unwrap()
    }
}

impl proto::Protobuf for Vote {
    type Proto = p2p_proto::Vote;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        let vote_type = proto_to_common_vote_type(proto.vote_type());

        let extension = proto
            .extension
            .map(|data| -> Result<_, proto::Error> {
                let extension = Extension::from(data.data);
                let signature = data.signature.ok_or_else(|| {
                    proto::Error::missing_field::<Self::Proto>("extension.signature")
                })?;

                Ok(SignedExtension::new(
                    extension,
                    Signature::from_proto(signature)?,
                ))
            })
            .transpose()?;

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
            extension,
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
            extension: self
                .extension
                .as_ref()
                .map(|ext| -> Result<_, proto::Error> {
                    Ok(p2p_proto::Extension {
                        data: ext.message.data.clone(),
                        signature: Some(ext.signature.to_proto()?),
                    })
                })
                .transpose()?,
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
