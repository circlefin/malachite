// use bytes::Bytes;
use malachitebft_core_types::Round;
// use malachitebft_proto as proto;
// use malachitebft_starknet_p2p_proto as p2p_proto;

use crate::{Address, Hash, Height};

/// A proposal for a value in a round
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    pub height: Height,
    pub round: Round,
    pub value_id: Hash,
    pub pol_round: Round,
    pub proposer: Address,
}

impl Proposal {
    pub fn new(
        height: Height,
        round: Round,
        value_id: Hash,
        pol_round: Round,
        proposer: Address,
    ) -> Self {
        Self {
            height,
            round,
            value_id,
            pol_round,
            proposer,
        }
    }
}
