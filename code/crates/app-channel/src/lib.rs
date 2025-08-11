//! Channel-based interface for Malachite applications.

// TODO: Enforce proper documentation
// #![warn(
//     missing_docs,
//     clippy::empty_docs,
//     clippy::missing_errors_doc,
//     rustdoc::broken_intra_doc_links,
//     rustdoc::missing_crate_level_docs,
//     rustdoc::missing_doc_code_examples
// )]

pub use malachitebft_app as app;

mod connector;
mod spawn;

mod msgs;
pub use msgs::{AppMsg, Channels, ConsensusMsg, NetworkMsg, Reply};

pub mod app_msg {
    pub use crate::msgs::ConsensusReady;
    pub use crate::msgs::Decided;
    pub use crate::msgs::ExtendVote;
    pub use crate::msgs::GetDecidedValue;
    pub use crate::msgs::GetHistoryMinHeight;
    pub use crate::msgs::GetValidatorSet;
    pub use crate::msgs::GetValue;
    pub use crate::msgs::ProcessSyncedValue;
    pub use crate::msgs::ReceivedProposalPart;
    pub use crate::msgs::RestreamProposal;
    pub use crate::msgs::StartedRound;
    pub use crate::msgs::VerifyVoteExtension;
}

mod run;
pub use run::start_engine;
