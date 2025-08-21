use std::collections::BTreeMap;

use derive_where::derive_where;
use malachitebft_core_types::Context;

/// All the types used in the state dump.
pub mod types {
    pub use malachitebft_core_consensus::full_proposal::{
        Entry as FullProposalEntry, FullProposal, FullProposalKeeper,
    };
    pub use malachitebft_core_consensus::util::bounded_queue::BoundedQueue;
    pub use malachitebft_core_consensus::Input as ConsensusInput;
    pub use malachitebft_core_consensus::Params as ConsensusParams;
    pub use malachitebft_core_driver::proposal_keeper::EvidenceMap as ProposalEvidenceMap;
    pub use malachitebft_core_driver::proposal_keeper::PerRound as ProposalPerRound;
    pub use malachitebft_core_state_machine::state::State;
    pub use malachitebft_core_state_machine::state::Step;
    pub use malachitebft_core_types::EnterRoundCertificate;
    pub use malachitebft_core_types::ValuePayload;
    pub use malachitebft_core_types::{Round, SignedVote, ThresholdParams};
    pub use malachitebft_core_votekeeper::evidence::EvidenceMap as VoteEvidenceMap;
    pub use malachitebft_core_votekeeper::keeper::PerRound as VotePerRound;
}

use self::types::*;

/// The state of the vote keeper, which keeps track of votes and misbehavior evidence.
#[derive_where(Debug, Clone)]
pub struct VoteKeeperState<Ctx: Context> {
    /// The votes that were received in each round so far
    pub votes: BTreeMap<Round, VotePerRound<Ctx>>,

    /// Misbehavior evidence for voting
    pub evidence: VoteEvidenceMap<Ctx>,
}

/// The state of the proposal keeper, which keeps track of proposals and proposal-related misbehavior evidence.
#[derive_where(Debug, Clone)]
pub struct ProposalKeeperState<Ctx: Context> {
    /// The proposals that were received in each round so far
    pub proposals: BTreeMap<Round, ProposalPerRound<Ctx>>,

    /// Misbehavior evidence for proposals
    pub evidence: ProposalEvidenceMap<Ctx>,
}

/// A dump of the current state of the consensus engine.
#[derive_where(Debug, Clone)]
pub struct StateDump<Ctx: Context> {
    /// The state of the core state machine
    pub consensus: State<Ctx>,

    /// The address of the node
    pub address: Ctx::Address,

    /// The proposer for the current round, None for round nil
    pub proposer: Option<Ctx::Address>,

    /// The consensus parameters
    pub params: ConsensusParams<Ctx>,

    /// The validator set at the current height
    pub validator_set: Ctx::ValidatorSet,

    /// The state of the vote keeper
    pub vote_keeper: VoteKeeperState<Ctx>,

    /// The state of the proposal keeper
    pub proposal_keeper: ProposalKeeperState<Ctx>,

    /// The proposals to decide on.
    pub full_proposal_keeper: FullProposalKeeper<Ctx>,

    /// Last prevote broadcasted by this node
    pub last_signed_prevote: Option<SignedVote<Ctx>>,

    /// Last precommit broadcasted by this node
    pub last_signed_precommit: Option<SignedVote<Ctx>>,

    /// The certificate that justifies moving to the `enter_round` specified in the certificate
    pub round_certificate: Option<EnterRoundCertificate<Ctx>>,

    /// A queue of inputs for higher heights, buffered for future processing
    pub input_queue: BoundedQueue<Ctx::Height, ConsensusInput<Ctx>>,
}

impl<Ctx: Context> StateDump<Ctx> {
    pub fn new(state: &super::ConsensusState<Ctx>) -> Self {
        Self {
            consensus: state.driver.round_state().clone(),
            address: state.address().clone(),
            proposer: state.driver.proposer_address().cloned(),
            params: state.params.clone(),
            validator_set: state.validator_set().clone(),
            vote_keeper: VoteKeeperState {
                votes: state.driver.votes().all_rounds().clone(),
                evidence: state.driver.votes().evidence().clone(),
            },
            proposal_keeper: ProposalKeeperState {
                proposals: state.driver.proposals().all_rounds().clone(),
                evidence: state.driver.proposals().evidence().clone(),
            },
            full_proposal_keeper: state.full_proposal_keeper.clone(),
            last_signed_prevote: state.last_signed_prevote.clone(),
            last_signed_precommit: state.last_signed_precommit.clone(),
            round_certificate: state.driver.round_certificate().cloned(),
            input_queue: state.input_queue.clone(),
        }
    }
}
