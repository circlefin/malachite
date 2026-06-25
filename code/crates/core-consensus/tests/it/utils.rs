use informalsystems_malachitebft_core_consensus::{Input, WalEntry};
use malachitebft_core_types::{Context, Timeout};

/// Convert a WAL entry back into a core-consensus input for deterministic replay.
pub fn wal_entry_to_input<Ctx: Context>(entry: WalEntry<Ctx>) -> Input<Ctx> {
    match entry {
        WalEntry::ConsensusMsg(msg) => match msg {
            informalsystems_malachitebft_core_consensus::SignedConsensusMsg::Vote(v) => {
                Input::Vote(v)
            }
            informalsystems_malachitebft_core_consensus::SignedConsensusMsg::Proposal(p) => {
                Input::Proposal(p)
            }
        },
        WalEntry::Timeout(timeout) => Input::TimeoutElapsed(timeout),
        WalEntry::ProposedValue(v) => {
            // For now, treat replay as if the value arrived from consensus gossip.
            // (Scenarios that require ValueOrigin::Sync will be added in follow-up PRs.)
            Input::ProposedValue(v, malachitebft_core_types::ValueOrigin::Consensus)
        }
    }
}

pub fn propose_timeout(round: u32) -> Timeout {
    Timeout::propose(malachitebft_core_types::Round::new(round))
}
