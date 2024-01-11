use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use malachite_common::{
    Context, Proposal, Round, Timeout, TimeoutStep, Validator, ValidatorSet, Vote,
};
use malachite_round::input::Input as RoundInput;
use malachite_round::output::Output as RoundOutput;
use malachite_round::state::State as RoundState;
use malachite_round::state_machine::Info;
use malachite_vote::keeper::VoteKeeper;
use malachite_vote::ThresholdParams;

use crate::input::Input;
use crate::output::Output;
use crate::Error;
use crate::ProposerSelector;
use crate::Validity;

/// Driver for the state machine of the Malachite consensus engine at a given height.
pub struct Driver<Ctx>
where
    Ctx: Context,
{
    /// The context of the consensus engine,
    /// for defining the concrete data types and signature scheme.
    pub ctx: Ctx,

    /// The proposer selector.
    pub proposer_selector: Box<dyn ProposerSelector<Ctx>>,

    /// The address of the node.
    pub address: Ctx::Address,

    /// The validator set at the current height
    pub validator_set: Ctx::ValidatorSet,

    /// The vote keeper.
    pub vote_keeper: VoteKeeper<Ctx>,

    /// The state of the round state machine.
    pub round_state: RoundState<Ctx>,

    /// The proposal to decide on, if any.
    pub proposal: Option<Ctx::Proposal>,

    /// The pending input to be processed next, if any.
    pub pending_input: Option<(Round, RoundInput<Ctx>)>,
}

impl<Ctx> Driver<Ctx>
where
    Ctx: Context,
{
    /// Create a new `Driver` instance for the given height.
    ///
    /// This instance is only valid for a single height
    /// and should be discarded and re-created for the next height.
    pub fn new(
        ctx: Ctx,
        height: Ctx::Height,
        proposer_selector: impl ProposerSelector<Ctx> + 'static,
        validator_set: Ctx::ValidatorSet,
        address: Ctx::Address,
        threshold_params: ThresholdParams,
    ) -> Self {
        let votes = VoteKeeper::new(validator_set.total_voting_power(), threshold_params);

        Self {
            ctx,
            proposer_selector: Box::new(proposer_selector),
            address,
            validator_set,
            vote_keeper: votes,
            round_state: RoundState::new(height, Round::Nil),
            proposal: None,
            pending_input: None,
        }
    }

    /// Return the height of the consensus.
    pub fn height(&self) -> Ctx::Height {
        self.round_state.height
    }

    /// Return the current round we are at.
    pub fn round(&self) -> Round {
        self.round_state.round
    }

    /// Return the proposer for the current round.
    pub fn get_proposer(&self, round: Round) -> Result<&Ctx::Validator, Error<Ctx>> {
        let address = self
            .proposer_selector
            .select_proposer(round, &self.validator_set);

        let proposer = self
            .validator_set
            .get_by_address(&address)
            .ok_or_else(|| Error::ProposerNotFound(address))?;

        Ok(proposer)
    }

    /// Process the given input, returning the outputs to be broadcast to the network.
    pub fn process(&mut self, msg: Input<Ctx>) -> Result<Vec<Output<Ctx>>, Error<Ctx>> {
        let round_output = match self.apply(msg)? {
            Some(msg) => msg,
            None => return Ok(Vec::new()),
        };

        let output = self.lift_output(round_output);
        let mut outputs = vec![output];

        self.process_pending(&mut outputs)?;

        Ok(outputs)
    }

    /// Process the pending input, if any.
    fn process_pending(&mut self, outputs: &mut Vec<Output<Ctx>>) -> Result<(), Error<Ctx>> {
        while let Some((round, input)) = self.pending_input.take() {
            if let Some(round_output) = self.apply_input(round, input)? {
                let output = self.lift_output(round_output);
                outputs.push(output);
            };
        }

        Ok(())
    }

    /// Convert an output of the round state machine to the output type of the driver.
    fn lift_output(&mut self, round_output: RoundOutput<Ctx>) -> Output<Ctx> {
        match round_output {
            RoundOutput::NewRound(round) => Output::NewRound(self.height(), round),

            RoundOutput::Proposal(proposal) => Output::Propose(proposal),

            RoundOutput::Vote(vote) => Output::Vote(vote),

            RoundOutput::ScheduleTimeout(timeout) => Output::ScheduleTimeout(timeout),

            RoundOutput::GetValueAndScheduleTimeout(round, timeout) => {
                Output::GetValueAndScheduleTimeout(round, timeout)
            }

            RoundOutput::Decision(value) => Output::Decide(value.round, value.value),
        }
    }

    /// Apply the given input to the state machine, returning the output, if any.
    fn apply(&mut self, input: Input<Ctx>) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        match input {
            Input::NewRound(height, round) => self.apply_new_round(height, round),
            Input::ProposeValue(round, value) => self.apply_propose_value(round, value),
            Input::Proposal(proposal, validity) => self.apply_proposal(proposal, validity),
            Input::Vote(vote) => self.apply_vote(vote),
            Input::TimeoutElapsed(timeout) => self.apply_timeout(timeout),
        }
    }

    fn apply_new_round(
        &mut self,
        height: Ctx::Height,
        round: Round,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        if self.height() == height {
            // If it's a new round for same height, just reset the round, keep the valid and locked values
            self.round_state.round = round;
        } else {
            self.round_state = RoundState::new(height, round);
        }

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
        proposal: Ctx::Proposal,
        validity: Validity,
    ) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        let round = proposal.round();

        match self.multiplex_proposal(proposal, validity) {
            Some(round_input) => self.apply_input(round, round_input),
            None => Ok(None),
        }
    }

    fn apply_vote(&mut self, vote: Ctx::Vote) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        let validator = self
            .validator_set
            .get_by_address(vote.validator_address())
            .ok_or_else(|| Error::ValidatorNotFound(vote.validator_address().clone()))?;

        let vote_round = vote.round();
        let current_round = self.round();

        let vote_output =
            self.vote_keeper
                .apply_vote(vote, validator.voting_power(), current_round);

        let Some(vote_output) = vote_output else {
            return Ok(None);
        };

        let round_input = self.multiplex_vote_threshold(vote_output);
        self.apply_input(vote_round, round_input)
    }

    fn apply_timeout(&mut self, timeout: Timeout) -> Result<Option<RoundOutput<Ctx>>, Error<Ctx>> {
        let input = match timeout.step {
            TimeoutStep::Propose => RoundInput::TimeoutPropose,
            TimeoutStep::Prevote => RoundInput::TimeoutPrevote,
            TimeoutStep::Precommit => RoundInput::TimeoutPrecommit,
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
        let current_step = round_state.step;

        let proposer = self.get_proposer(round_state.round)?;
        let info = Info::new(input_round, &self.address, proposer.address());

        // Apply the input to the round state machine
        let transition = round_state.apply(&info, input);

        let pending_step = transition.next_state.step;

        if current_step != pending_step {
            let pending_input = self.multiplex_step_change(pending_step, input_round);

            self.pending_input = pending_input.map(|input| (input_round, input));
        }

        // Update state
        self.round_state = transition.next_state;

        // Return output, if any
        Ok(transition.output)
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
            .field("votes", &self.vote_keeper)
            .field("proposal", &self.proposal)
            .field("round_state", &self.round_state)
            .finish()
    }
}
