use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use malachitebft_core_state_machine::input::Input as RoundInput;
use malachitebft_core_state_machine::output::Output as RoundOutput;
use malachitebft_core_state_machine::state::{RoundValue, State as RoundState, Step};
use malachitebft_core_state_machine::state_machine::Info;
use malachitebft_core_types::{
    CommitCertificate, Context, Proposal, Round, SignedProposal, SignedVote, Timeout, TimeoutKind,
    Validator, ValidatorSet, Validity, ValueId, Vote,
};
use malachitebft_core_votekeeper::keeper::VoteKeeper;

use crate::input::Input;
use crate::output::Output;
use crate::proposal_keeper::{EvidenceMap, ProposalKeeper};
use crate::Error;
use crate::ThresholdParams;

/// Driver for the state machine of the Malachite consensus engine at a given height.
pub struct Driver<Ctx>
where
    Ctx: Context,
{
    /// The context of the consensus engine,
    /// for defining the concrete data types and signature scheme.
    #[allow(dead_code)]
    ctx: Ctx,

    /// The address of the node.
    address: Ctx::Address,

    /// Quorum thresholds
    threshold_params: ThresholdParams,

    /// The validator set at the current height
    validator_set: Ctx::ValidatorSet,

    /// The proposer for the current round, None for round nil.
    proposer: Option<Ctx::Address>,

    /// The proposals to decide on.
    pub(crate) proposal_keeper: ProposalKeeper<Ctx>,

    /// The vote keeper.
    pub(crate) vote_keeper: VoteKeeper<Ctx>,

    /// The certificate keeper
    pub(crate) certificates: Vec<CommitCertificate<Ctx>>,

    /// The state of the round state machine.
    pub(crate) round_state: RoundState<Ctx>,

    /// The pending inputs to be processed next, if any.
    /// The first element of the tuple is the round at which that input has been emitted.
    pending_inputs: Vec<(Round, RoundInput<Ctx>)>,
}

impl<Ctx> Driver<Ctx>
where
    Ctx: Context,
{
    /// Create a new `Driver` instance for the given height.
    ///
    /// Called when consensus is started and initialized with the first height.
    /// Re-initialization for subsequent heights is done using `move_to_height()`.
    pub fn new(
        ctx: Ctx,
        height: Ctx::Height,
        validator_set: Ctx::ValidatorSet,
        address: Ctx::Address,
        threshold_params: ThresholdParams,
    ) -> Self {
        let proposal_keeper = ProposalKeeper::new();
        let vote_keeper = VoteKeeper::new(validator_set.clone(), threshold_params);
        let round_state = RoundState::new(height, Round::Nil);

        Self {
            ctx,
            address,
            threshold_params,
            validator_set,
            proposal_keeper,
            vote_keeper,
            round_state,
            proposer: None,
            pending_inputs: vec![],
            certificates: vec![],
        }
    }

    /// Reset votes, round state, pending input
    /// and move to new height with the given validator set.
    pub fn move_to_height(&mut self, height: Ctx::Height, validator_set: Ctx::ValidatorSet) {
        // Reset the proposal keeper
        let proposal_keeper = ProposalKeeper::new();

        // Reset the vote keeper
        let vote_keeper = VoteKeeper::new(validator_set.clone(), self.threshold_params);

        // Reset the round state
        let round_state = RoundState::new(height, Round::Nil);

        self.validator_set = validator_set;
        self.proposal_keeper = proposal_keeper;
        self.vote_keeper = vote_keeper;
        self.round_state = round_state;
        self.pending_inputs = vec![];
        self.certificates = vec![];
    }

    /// Return the height of the consensus.
    pub fn height(&self) -> Ctx::Height {
        self.round_state.height
    }

    /// Return the current round we are at.
    pub fn round(&self) -> Round {
        self.round_state.round
    }

    /// Return the current step within the round we are at.
    pub fn step(&self) -> Step {
        self.round_state.step
    }

    /// Returns true if the current step is propose.
    pub fn step_is_propose(&self) -> bool {
        self.round_state.step == Step::Propose
    }

    /// Returns true if the current step is prevote.
    pub fn step_is_prevote(&self) -> bool {
        self.round_state.step == Step::Prevote
    }

    /// Returns true if the current step is precommit.
    pub fn step_is_precommit(&self) -> bool {
        self.round_state.step == Step::Precommit
    }

    /// Returns true if the current step is commit.
    pub fn step_is_commit(&self) -> bool {
        self.round_state.step == Step::Commit
    }

    /// Return the valid value (the value for which we saw a polka) for the current round, if any.
    pub fn valid_value(&self) -> Option<&RoundValue<Ctx::Value>> {
        self.round_state.valid.as_ref()
    }

    /// Return a reference to the votekeper
    pub fn votes(&self) -> &VoteKeeper<Ctx> {
        &self.vote_keeper
    }

    /// Return the state for the current round.
    pub fn round_state(&self) -> &RoundState<Ctx> {
        &self.round_state
    }

    /// Return the address of the node.
    pub fn address(&self) -> &Ctx::Address {
        &self.address
    }

    /// Return the validator set for this height.
    pub fn validator_set(&self) -> &Ctx::ValidatorSet {
        &self.validator_set
    }

    /// Return recorded evidence of equivocation for this height.
    pub fn evidence(&self) -> &EvidenceMap<Ctx> {
        self.proposal_keeper.evidence()
    }

    /// Return the proposer for the current round.
    pub fn get_proposer(&self) -> Result<&Ctx::Validator, Error<Ctx>> {
        if let Some(proposer) = &self.proposer {
            let proposer = self
                .validator_set
                .get_by_address(proposer)
                .ok_or_else(|| Error::ProposerNotFound(proposer.clone()))?;

            Ok(proposer)
        } else {
            Err(Error::NoProposer(self.height(), self.round()))
        }
    }

    /// Get a commit certificate for the given round and value id.
    pub fn get_certificate(
        &self,
        round: Round,
        value_id: ValueId<Ctx>,
    ) -> Option<&CommitCertificate<Ctx>> {
        self.certificates
            .iter()
            .find(|c| c.round == round && c.value_id == value_id)
    }

    /// Process the given input, returning the outputs to be broadcast to the network.
    pub fn process(&mut self, msg: Input<Ctx>) -> Result<Vec<Output<Ctx>>, Error<Ctx>> {
        let round_output = match self.apply(msg)? {
            Some(msg) => msg,
            None => return Ok(Vec::new()),
        };

        let mut outputs = vec![];

        // Lift the round state machine output to one or more driver outputs
        self.lift_output(round_output, &mut outputs);

        // Apply the pending inputs, if any, and lift their outputs
        while !self.pending_inputs.is_empty() {
            let new_pending = core::mem::take(&mut self.pending_inputs);
            for (round, input) in new_pending {
                if let Some(output) = self.apply_input(round, input)? {
                    self.lift_output(output, &mut outputs)
                }
            }
        }

        Ok(outputs)
    }

    /// Convert the output of the round state machine to the output type of the driver.
    fn lift_output(&mut self, round_output: RoundOutput<Ctx>, outputs: &mut Vec<Output<Ctx>>) {
        match round_output {
            RoundOutput::NewRound(round) => outputs.push(Output::NewRound(self.height(), round)),

            RoundOutput::Proposal(proposal) => outputs.push(Output::Propose(proposal)),

            RoundOutput::Vote(vote) => outputs.push(Output::Vote(vote)),

            RoundOutput::ScheduleTimeout(timeout) => outputs.push(Output::ScheduleTimeout(timeout)),

            RoundOutput::GetValueAndScheduleTimeout(height, round, timeout) => {
                outputs.push(Output::ScheduleTimeout(timeout));
                outputs.push(Output::GetValue(height, round, timeout));
            }

            RoundOutput::Decision(round, proposal) => outputs.push(Output::Decide(round, proposal)),
        }
    }

    /// Apply the given input to the state machine, returning the output, if any.
    fn apply(&mut self, input: Input<Ctx>) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        match input {
            Input::CommitCertificate(certificate) => self.apply_commit_certificate(certificate),
            Input::NewRound(height, round, proposer) => {
                self.apply_new_round(height, round, proposer)
            }
            Input::ProposeValue(round, value) => self.apply_propose_value(round, value),
            Input::Proposal(proposal, validity) => self.apply_proposal(proposal, validity),
            Input::Vote(vote) => self.apply_vote(vote),
            Input::TimeoutElapsed(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_commit_certificate(
        &mut self,
        certificate: CommitCertificate<Ctx>,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        if self.height() != certificate.height {
            return Err(Error::InvalidCertificateHeight {
                certificate_height: certificate.height,
                consensus_height: self.height(),
            });
        }

        let round = certificate.round;

        match self.store_and_multiplex_certificate(certificate) {
            Some(round_input) => self.apply_input(round, round_input),
            None => Ok(None),
        }
    }

    fn apply_new_round(
        &mut self,
        height: Ctx::Height,
        round: Round,
        proposer: Ctx::Address,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        if self.height() == height {
            // If it's a new round for same height, just reset the round, keep the valid and locked values
            self.round_state.round = round;
        } else {
            self.round_state = RoundState::new(height, round);
        }

        // Update the proposer for the new round
        self.proposer = Some(proposer);

        self.apply_input(round, RoundInput::NewRound(round))
    }

    fn apply_propose_value(
        &mut self,
        round: Round,
        value: Ctx::Value,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        self.apply_input(round, RoundInput::ProposeValue(value))
    }

    fn apply_proposal(
        &mut self,
        proposal: SignedProposal<Ctx>,
        validity: Validity,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        if self.height() != proposal.height() {
            return Err(Error::InvalidProposalHeight {
                proposal_height: proposal.height(),
                consensus_height: self.height(),
            });
        }

        let round = proposal.round();

        match self.store_and_multiplex_proposal(proposal, validity) {
            Some(round_input) => self.apply_input(round, round_input),
            None => Ok(None),
        }
    }

    fn apply_vote(
        &mut self,
        vote: SignedVote<Ctx>,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        if self.height() != vote.height() {
            return Err(Error::InvalidVoteHeight {
                vote_height: vote.height(),
                consensus_height: self.height(),
            });
        }

        if self
            .validator_set
            .get_by_address(vote.validator_address())
            .is_none()
        {
            return Err(Error::ValidatorNotFound(vote.validator_address().clone()));
        }

        let vote_round = vote.round();

        let Some(output) = self.vote_keeper.apply_vote(vote, self.round()) else {
            return Ok(None);
        };

        let round_input = self.multiplex_vote_threshold(output, vote_round);

        if round_input == RoundInput::NoInput {
            return Ok(None);
        }

        self.apply_input(vote_round, round_input)
    }

    fn apply_timeout(&mut self, timeout: Timeout) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        let input = match timeout.kind {
            TimeoutKind::Propose => RoundInput::TimeoutPropose,
            TimeoutKind::Prevote => RoundInput::TimeoutPrevote,
            TimeoutKind::Precommit => RoundInput::TimeoutPrecommit,

            // The driver never receives a commit or time limit timeout, so we can just ignore it.
            TimeoutKind::Commit => return Ok(None),
            TimeoutKind::PrevoteTimeLimit => return Ok(None),
            TimeoutKind::PrecommitTimeLimit => return Ok(None),
        };

        self.apply_input(timeout.round, input)
    }

    /// Apply the input, update the state.
    fn apply_input(
        &mut self,
        input_round: Round,
        input: RoundInput<Ctx>,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        let round_state = core::mem::take(&mut self.round_state);

        let previous_step = round_state.step;

        let proposer = self.get_proposer()?;
        let info = Info::new(input_round, &self.address, proposer.address());

        // Apply the input to the round state machine
        let transition = round_state.apply(&info, input);

        // Update state
        self.round_state = transition.next_state;

        if previous_step != self.round_state.step && self.round_state.step != Step::Unstarted {
            let pending_inputs = self.multiplex_step_change(input_round);

            self.pending_inputs = pending_inputs
                .into_iter()
                .map(|input| (input_round, input))
                .collect();
        }

        // Return output, if any
        Ok(transition.output)
    }

    /// Return the traces logged during execution.
    #[cfg(feature = "debug")]
    pub fn get_traces(&self) -> &[malachitebft_core_state_machine::traces::Trace<Ctx>] {
        self.round_state.get_traces()
    }
}

impl<Ctx> fmt::Debug for Driver<Ctx>
where
    Ctx: Context,
{
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Driver")
            .field("address", &self.address)
            .field("validator_set", &self.validator_set)
            .field("proposal", &self.proposal_keeper)
            .field("votes", &self.vote_keeper)
            .field("round_state", &self.round_state)
            .finish()
    }
}
