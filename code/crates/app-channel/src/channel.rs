use std::time::Duration;

use bytes::Bytes;
use derive_where::derive_where;
use libp2p_identity::PeerId;
use tokio::sync::oneshot;

use malachite_actors::host::LocallyProposedValue;
use malachite_actors::util::streaming::StreamMessage;
use malachite_blocksync::SyncedBlock;
use malachite_common::{CommitCertificate, Context, Round, ValueId};
use malachite_consensus::ProposedValue;

/// Messages that will be sent on the channel.
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
        reply_to: oneshot::Sender<Option<SyncedBlock<Ctx>>>,
    },

    // Synced block
    ProcessSyncedBlock {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        block_bytes: Bytes,
        reply_to: oneshot::Sender<ProposedValue<Ctx>>,
    },
}

#[derive_where(Debug)]
pub enum ConsensusMsg<Ctx: Context> {
    StartHeight(Ctx::Height),
}

use malachite_actors::consensus::Msg as ConsensusActorMsg;

impl<Ctx: Context> From<ConsensusMsg<Ctx>> for ConsensusActorMsg<Ctx> {
    fn from(msg: ConsensusMsg<Ctx>) -> ConsensusActorMsg<Ctx> {
        match msg {
            ConsensusMsg::StartHeight(height) => ConsensusActorMsg::StartHeight(height),
        }
    }
}
