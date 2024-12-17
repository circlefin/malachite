use derive_where::derive_where;

use malachite_core_types::{Context, Round};

/// The type of errors that can be yielded by the `Driver`.
#[derive_where(Clone, Debug, PartialEq, Eq)]
#[derive(thiserror::Error)]
pub enum Error<Ctx>
where
    Ctx: Context,
{
    /// No proposer was set for this round
    #[error("No proposer set for height {0} at round {1}")]
    NoProposer(Ctx::Height, Round),

    /// Proposer not found
    #[error("Proposer not found: {0}")]
    ProposerNotFound(Ctx::Address),

    /// Validator not found in validator set
    #[error("Validator not found: {0}")]
    ValidatorNotFound(Ctx::Address),

    /// Received a proposal for another height
    #[error("Received proposal for height {proposal_height} different from consensus height {consensus_height}")]
    InvalidProposalHeight {
        /// Proposal height
        proposal_height: Ctx::Height,
        /// Consensus height
        consensus_height: Ctx::Height,
    },

    /// Received a vote for another height
    #[error(
        "Received vote for height {vote_height} different from consensus height {consensus_height}"
    )]
    InvalidVoteHeight {
        /// Vote height
        vote_height: Ctx::Height,
        /// Consensus height
        consensus_height: Ctx::Height,
    },

    /// Received a certificate for another height
    #[error("Received certificate for height {certificate_height} different from consensus height {consensus_height}")]
    InvalidCertificateHeight {
        /// Certificate height
        certificate_height: Ctx::Height,
        /// Consensus height
        consensus_height: Ctx::Height,
    },
}
