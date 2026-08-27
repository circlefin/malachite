use derive_where::derive_where;
use malachitebft_core_types::{
    Context, PolkaCertificate, RoundCertificate, SignedProposal, SignedVote, Timeout, ValueOrigin,
    ValueResponse, VoteExtensionPolicy,
};
use std::time::Duration;

use crate::types::{LocallyProposedValue, ProposedValue};

/// Inputs to be handled by the consensus process.
///
/// **Persistence to the Write-Ahead Log**: only a subset of these variants is appended to the
/// WAL — the ones whose effects on the driver's equivocation guards (`last_prevote`,
/// `last_precommit`, `valid`, `locked`, stored certificates) must survive a crash. The
/// authoritative list lives in the WAL codec (`engine::wal::entry::encode_entry`); variants
/// that must NOT be persisted are documented inline below and rejected at the codec layer.
#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Input<Ctx>
where
    Ctx: Context,
{
    /// Start consensus for the given height with an optional validator set update.
    /// The boolean indicates whether this is a restart of consensus for the given height.
    /// The optional Duration is the target time for this height.
    ///
    /// **Not persisted**: engine-layer envelope; the host rebuilds the parameters on every
    /// restart, so persisting them is unnecessary.
    ///
    /// # Ordering contract
    ///
    /// This input MUST only be provided after the previous height has emitted
    /// [`Effect::Finalize`][crate::effect::Effect::Finalize] and the application has
    /// finished processing it. Driving `StartHeight` while the previous height is
    /// still in its finalization window causes the state machine to flush
    /// `Effect::Finalize` for the previous height before starting the new one — so
    /// applications that violate this ordering must be prepared to receive that
    /// `Effect::Finalize` *after* they have already requested the new height.
    StartHeight(
        Ctx::Height,
        Ctx::ValidatorSet,
        bool,
        Option<Duration>,
        VoteExtensionPolicy,
    ),

    /// Process a vote received over the network.
    Vote(SignedVote<Ctx>),

    /// Process a Proposal message received over the network
    ///
    /// This input MUST only be provided when `ValuePayload` is set to `ProposalOnly` or `ProposalAndParts`,
    /// i.e. when consensus runs in a mode where the proposer sends a Proposal consensus message over the network.
    Proposal(SignedProposal<Ctx>),

    /// Process a PolkaCertificate message received over the network
    PolkaCertificate(PolkaCertificate<Ctx>),

    /// Process a RoundCertificate message received over the network.
    ///
    /// **Not persisted**: round certificates dissolve into per-vote driver inputs that are not
    /// individually WAL'd today, so the certificate itself is not persisted either
    /// (cf. issue #1445).
    RoundCertificate(RoundCertificate<Ctx>),

    /// Propose the given value.
    ///
    /// This input MUST only be provided when we are the proposer for the current round.
    ///
    /// **Not persisted**: locally-proposed values are converted to a `ProposedValue` input
    /// downstream and persisted in that form.
    Propose(LocallyProposedValue<Ctx>),

    /// A timeout has elapsed.
    TimeoutElapsed(Timeout),

    /// We have received the full proposal for the current round.
    ///
    /// The origin denotes whether the value was received via consensus gossip or via the sync protocol.
    ProposedValue(ProposedValue<Ctx>, ValueOrigin),

    /// We have received a synced value via the sync protocol.
    ///
    /// **Not persisted**: sync-protocol wire envelope; the verified `CommitCertificate` it
    /// carries is the artifact that warrants WAL coverage, tracked separately
    /// (cf. issue #1445).
    SyncValueResponse(ValueResponse<Ctx>),
}
