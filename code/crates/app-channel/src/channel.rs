use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use malachite_engine::consensus::Msg as ConsensusActorMsg;
use malachite_engine::network::Msg as NetworkActorMsg;

use crate::app::types::core::{CommitCertificate, Context, Round, ValueId};
use crate::app::types::streaming::StreamMessage;
use crate::app::types::sync::DecidedValue;
use crate::app::types::{LocallyProposedValue, PeerId, ProposedValue};

pub type Reply<T> = oneshot::Sender<T>;

/// Channels created for application consumption
pub struct Channels<Ctx: Context> {
    pub consensus: mpsc::Receiver<AppMsg<Ctx>>,
    pub network: mpsc::Sender<NetworkMsg<Ctx>>,
}

/// Messages sent from consensus to the application.
#[derive_where(Debug)]
pub enum AppMsg<Ctx: Context> {
    /// Consensus is ready
    ConsensusReady { reply: Reply<ConsensusMsg<Ctx>> },

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
        timeout: Duration,
        reply: Reply<LocallyProposedValue<Ctx>>,
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
    GetHistoryMinHeight { reply: Reply<Ctx::Height> },

    /// ProposalPart received <-- consensus <-- gossip
    ReceivedProposalPart {
        from: PeerId,
        part: StreamMessage<Ctx::ProposalPart>,
        reply: Reply<ProposedValue<Ctx>>,
    },

    /// Get the validator set at a given height
    GetValidatorSet {
        height: Ctx::Height,
        reply: Reply<Ctx::ValidatorSet>,
    },

    // Consensus has decided on a value
    Decided {
        certificate: CommitCertificate<Ctx>,
        reply: Reply<ConsensusMsg<Ctx>>,
    },

    // Retrieve decided block from the block store
    GetDecidedValue {
        height: Ctx::Height,
        reply: Reply<Option<DecidedValue<Ctx>>>,
    },

    // Synced block
    ProcessSyncedValue {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        value_bytes: Bytes,
        reply: Reply<ProposedValue<Ctx>>,
    },
}

/// Messages sent from the application to consensus.
#[derive_where(Debug)]
pub enum ConsensusMsg<Ctx: Context> {
    StartHeight(Ctx::Height, Ctx::ValidatorSet),
}

impl<Ctx: Context> From<ConsensusMsg<Ctx>> for ConsensusActorMsg<Ctx> {
    fn from(msg: ConsensusMsg<Ctx>) -> ConsensusActorMsg<Ctx> {
        match msg {
            ConsensusMsg::StartHeight(height, validator_set) => {
                ConsensusActorMsg::StartHeight(height, validator_set)
            }
        }
    }
}

/// Messages sent from the application to consensus gossip.
#[derive_where(Debug)]
pub enum NetworkMsg<Ctx: Context> {
    PublishProposalPart(StreamMessage<Ctx::ProposalPart>),
}

impl<Ctx: Context> From<NetworkMsg<Ctx>> for NetworkActorMsg<Ctx> {
    fn from(msg: NetworkMsg<Ctx>) -> NetworkActorMsg<Ctx> {
        match msg {
            NetworkMsg::PublishProposalPart(part) => NetworkActorMsg::PublishProposalPart(part),
        }
    }
}
