use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use tokio::sync::oneshot;

use crate::app::types::core::{CommitCertificate, Context, Round, ValueId};
use crate::app::types::streaming::StreamMessage;
use crate::app::types::sync::DecidedValue;
use crate::app::types::{LocallyProposedValue, PeerId, ProposedValue};

/// Messages sent from consensus to the application.
#[derive_where(Debug)]
pub enum AppMsg<Ctx: Context> {
    /// Consensus is ready
    ConsensusReady {
        reply_to: oneshot::Sender<ConsensusMsg<Ctx>>,
    },

    /// Consensus has started a new round.
    StartedRound {
        height: Ctx::Height,
        round: Round,
        proposer: Ctx::Address,
    },

    /// Request to build a local block/value from Driver
    GetValue {
        height: Ctx::Height,
        round: Round,
        timeout_duration: Duration,
        address: Ctx::Address,
        reply_to: oneshot::Sender<LocallyProposedValue<Ctx>>,
    },

    /// Request to restream an existing block/value from Driver
    RestreamValue {
        height: Ctx::Height,
        round: Round,
        valid_round: Round,
        address: Ctx::Address,
        value_id: ValueId<Ctx>,
    },

    /// Request the earliest block height in the block store
    GetEarliestBlockHeight {
        reply_to: oneshot::Sender<Ctx::Height>,
    },

    /// ProposalPart received <-- consensus <-- gossip
    ReceivedProposalPart {
        from: PeerId,
        part: StreamMessage<Ctx::ProposalPart>,
        reply_to: oneshot::Sender<ProposedValue<Ctx>>,
    },

    /// Get the validator set at a given height
    GetValidatorSet {
        height: Ctx::Height,
        reply_to: oneshot::Sender<Ctx::ValidatorSet>,
    },

    // Consensus has decided on a value
    Decided {
        certificate: CommitCertificate<Ctx>,
        reply_to: oneshot::Sender<ConsensusMsg<Ctx>>,
    },

    // Retrieve decided block from the block store
    GetDecidedBlock {
        height: Ctx::Height,
        reply_to: oneshot::Sender<Option<DecidedValue<Ctx>>>,
    },

    // Synced block
    ProcessSyncedValue {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        value_bytes: Bytes,
        reply_to: oneshot::Sender<ProposedValue<Ctx>>,
    },
}

/// Messages sent from the application to consensus.
#[derive_where(Debug)]
pub enum ConsensusMsg<Ctx: Context> {
    StartHeight(Ctx::Height, Ctx::ValidatorSet),
}

use malachite_engine::consensus::Msg as ConsensusActorMsg;

impl<Ctx: Context> From<ConsensusMsg<Ctx>> for ConsensusActorMsg<Ctx> {
    fn from(msg: ConsensusMsg<Ctx>) -> ConsensusActorMsg<Ctx> {
        match msg {
            ConsensusMsg::StartHeight(height, validator_set) => {
                ConsensusActorMsg::StartHeight(height, validator_set)
            }
        }
    }
}
