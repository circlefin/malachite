use std::collections::BTreeMap;

use derive_where::derive_where;

use malachitebft_core_types::Context;

pub use malachitebft_core_state_machine::state::State;
pub use malachitebft_core_state_machine::state::Step;
pub use malachitebft_core_types::EnterRoundCertificate;
pub use malachitebft_core_types::{Round, SignedVote, ThresholdParams};
pub use malachitebft_core_votekeeper::evidence::EvidenceMap;
pub use malachitebft_core_votekeeper::keeper::PerRound;

/// A dump of the current state of the consensus engine.
#[derive_where(Debug, Clone)]
pub struct StateDump<Ctx: Context> {
    /// The state of the core state machine
    pub consensus: State<Ctx>,

    /// The address of the node
    pub address: Ctx::Address,

    /// The proposer for the current round, None for round nil
    pub proposer: Option<Ctx::Address>,

    /// Quorum thresholds
    pub threshold_params: ThresholdParams,

    /// The validator set at the current height
    pub validator_set: Ctx::ValidatorSet,

    /// The votes that were received in each round so far
    pub votes: BTreeMap<Round, PerRound<Ctx>>,

    /// Misbehavior evidence
    pub evidence: EvidenceMap<Ctx>,

    /// Last prevote broadcasted by this node
    pub last_signed_prevote: Option<SignedVote<Ctx>>,

    /// Last precommit broadcasted by this node
    pub last_signed_precommit: Option<SignedVote<Ctx>>,

    /// The certificate that justifies moving to the `enter_round` specified in the certificate
    pub round_certificate: Option<EnterRoundCertificate<Ctx>>,
}

impl<Ctx: Context> StateDump<Ctx> {
    /// The height that consensus is currently at
    pub fn height(&self) -> Ctx::Height {
        self.consensus.height
    }

    /// The round that consensus is currently at
    pub fn round(&self) -> Round {
        self.consensus.round
    }

    /// The step that consensus is currently at
    pub fn step(&self) -> Step {
        self.consensus.step
    }
}

impl<Ctx: Context> From<&super::ConsensusState<Ctx>> for StateDump<Ctx> {
    fn from(state: &super::ConsensusState<Ctx>) -> Self {
        Self {
            consensus: state.driver.round_state().clone(),
            address: state.address().clone(),
            threshold_params: state.params.threshold_params,
            validator_set: state.validator_set().clone(),
            proposer: state.driver.proposer_address().cloned(),
            votes: state.driver.votes().all_rounds().clone(),
            evidence: state.driver.votes().evidence().clone(),
            last_signed_prevote: state.last_signed_prevote.clone(),
            last_signed_precommit: state.last_signed_precommit.clone(),
            round_certificate: state.driver.round_certificate().cloned(),
        }
    }
}
