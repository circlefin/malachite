use bytes::Bytes;
use libp2p_identity::PeerId;
use malachite_actors::host::LocallyProposedValue;
use malachite_actors::util::streaming::StreamMessage;
use malachite_blocksync::SyncedBlock;
use malachite_common::{CommitCertificate, Context, Round, ValueId};
use malachite_consensus::ProposedValue;
use std::time::Duration;
use tokio::sync::oneshot::Sender;

/// Messages that will be sent on the channel.
pub enum ChannelMsg<Ctx: Context> {
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
        reply_to: Sender<LocallyProposedValue<Ctx>>,
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
        reply_to: Sender<Ctx::Height>,
    },

    /// ProposalPart received <-- consensus <-- gossip
    ReceivedProposalPart {
        from: PeerId,
        part: StreamMessage<Ctx::ProposalPart>,
        reply_to: Sender<ProposedValue<Ctx>>,
    },

    /// Get the validator set at a given height
    GetValidatorSet {
        height: Ctx::Height,
        reply_to: Sender<Ctx::ValidatorSet>,
    },

    // Consensus has decided on a value
    Decided {
        certificate: CommitCertificate<Ctx>,
    },

    // Retrieve decided block from the block store
    GetDecidedBlock {
        height: Ctx::Height,
        reply_to: Sender<Option<SyncedBlock<Ctx>>>,
    },

    // Synced block
    ProcessSyncedBlock {
        height: Ctx::Height,
        round: Round,
        validator_address: Ctx::Address,
        block_bytes: Bytes,
        reply_to: Sender<ProposedValue<Ctx>>,
    },

    /// Consensus is ready
    ConsensusReady {},
}
