use alloc::collections::BTreeMap;

use malachite_round::state_machine::RoundData;

use malachite_common::{
    Context, Proposal, Round, SignedVote, Timeout, TimeoutStep, Validator, ValidatorSet, Value,
    Vote, VoteType,
};
use malachite_round::events::Event as RoundEvent;
use malachite_round::message::Message as RoundMessage;
use malachite_round::state::State as RoundState;
use malachite_vote::keeper::Message as VoteMessage;
use malachite_vote::keeper::VoteKeeper;
use malachite_vote::Threshold;

use crate::env::Env as DriverEnv;
use crate::event::Event;
use crate::message::Message;
use crate::Error;
use crate::ProposerSelector;
use crate::Validity;

/// Driver for the state machine of the Malachite consensus engine at a given height.
#[derive(Clone, Debug)]
pub struct Driver<Ctx, Env, PSel>
where
    Ctx: Context,
    Env: DriverEnv<Ctx>,
    PSel: ProposerSelector<Ctx>,
{
    pub ctx: Ctx,
    pub env: Env,
    pub proposer_selector: PSel,

    pub height: Ctx::Height,
    pub address: Ctx::Address,
    pub validator_set: Ctx::ValidatorSet,

    pub round: Round,
    pub votes: VoteKeeper<Ctx>,
    pub round_states: BTreeMap<Round, RoundState<Ctx>>,
}

impl<Ctx, Env, PSel> Driver<Ctx, Env, PSel>
where
    Ctx: Context,
    Env: DriverEnv<Ctx>,
    PSel: ProposerSelector<Ctx>,
{
    pub fn new(
        ctx: Ctx,
        env: Env,
        proposer_selector: PSel,
        height: Ctx::Height,
        validator_set: Ctx::ValidatorSet,
        address: Ctx::Address,
    ) -> Self {
        let votes = VoteKeeper::new(validator_set.total_voting_power());

        Self {
            ctx,
            env,
            proposer_selector,
            height,
            address,
            validator_set,
            round: Round::NIL,
            votes,
            round_states: BTreeMap::new(),
        }
    }

    async fn get_value(&self, round: Round) -> Option<Ctx::Value> {
        self.env.get_value(self.height.clone(), round).await
    }

    pub async fn execute(&mut self, msg: Event<Ctx>) -> Result<Option<Message<Ctx>>, Error<Ctx>> {
        let round_msg = match self.apply(msg).await? {
            Some(msg) => msg,
            None => return Ok(None),
        };

        let msg = match round_msg {
            RoundMessage::NewRound(round) => {
                // XXX: Check if there is an existing state?
                assert!(self.round < round);
                Message::NewRound(round)
            }

            RoundMessage::Proposal(proposal) => {
                // sign the proposal
                Message::Propose(proposal)
            }

            RoundMessage::Vote(vote) => {
                let signed_vote = self.ctx.sign_vote(vote);
                Message::Vote(signed_vote)
            }

            RoundMessage::ScheduleTimeout(timeout) => Message::ScheduleTimeout(timeout),

            RoundMessage::Decision(value) => {
                // TODO: update the state
                Message::Decide(value.round, value.value)
            }
        };

        Ok(Some(msg))
    }

    async fn apply(&mut self, msg: Event<Ctx>) -> Result<Option<RoundMessage<Ctx>>, Error<Ctx>> {
        match msg {
            Event::NewRound(round) => self.apply_new_round(round).await,
            Event::Proposal(proposal, validity) => {
                Ok(self.apply_proposal(proposal, validity).await)
            }
            Event::Vote(signed_vote) => self.apply_vote(signed_vote),
            Event::TimeoutElapsed(timeout) => Ok(self.apply_timeout(timeout)),
        }
    }

    async fn apply_new_round(
        &mut self,
        round: Round,
    ) -> Result<Option<RoundMessage<Ctx>>, Error<Ctx>> {
        let proposer_address = self
            .proposer_selector
            .select_proposer(round, &self.validator_set);

        let proposer = self
            .validator_set
            .get_by_address(&proposer_address)
            .ok_or_else(|| Error::ProposerNotFound(proposer_address.clone()))?;

        let event = if proposer.address() == &self.address {
            // We are the proposer
            // TODO: Schedule propose timeout

            let Some(value) = self.get_value(round).await else {
                return Err(Error::NoValueToPropose);
            };

            RoundEvent::NewRoundProposer(value)
        } else {
            RoundEvent::NewRound
        };

        assert!(self.round < round);
        self.round_states.insert(
            round,
            RoundState::default().new_round(self.height.clone(), round),
        );
        self.round = round;

        Ok(self.apply_event(round, event))
    }

    async fn apply_proposal(
        &mut self,
        proposal: Ctx::Proposal,
        validity: Validity,
    ) -> Option<RoundMessage<Ctx>> {
        // Check that there is an ongoing round
        let Some(round_state) = self.round_states.get(&self.round) else {
            // TODO: Add logging
            return None;
        };

        // Only process the proposal if there is no other proposal
        if round_state.proposal.is_some() {
            return None;
        }

        // Check that the proposal is for the current height and round
        if self.height != proposal.height() || self.round != proposal.round() {
            return None;
        }

        // TODO: Document
        if proposal.pol_round().is_defined() && proposal.pol_round() >= round_state.round {
            return None;
        }

        // TODO: Verify proposal signature (make some of these checks part of message validation)

        match proposal.pol_round() {
            Round::Nil => {
                // Is it possible to get +2/3 prevotes before the proposal?
                // Do we wait for our own prevote to check the threshold?
                let round = proposal.round();
                let event = if validity.is_valid() {
                    RoundEvent::Proposal(proposal)
                } else {
                    RoundEvent::ProposalInvalid
                };

                self.apply_event(round, event)
            }
            Round::Some(_)
                if self.votes.is_threshold_met(
                    &proposal.pol_round(),
                    VoteType::Prevote,
                    Threshold::Value(proposal.value().id()),
                ) =>
            {
                let round = proposal.round();
                let event = if validity.is_valid() {
                    RoundEvent::Proposal(proposal)
                } else {
                    RoundEvent::ProposalInvalid
                };

                self.apply_event(round, event)
            }
            _ => None,
        }
    }

    fn apply_vote(
        &mut self,
        signed_vote: SignedVote<Ctx>,
    ) -> Result<Option<RoundMessage<Ctx>>, Error<Ctx>> {
        let validator = self
            .validator_set
            .get_by_address(signed_vote.validator_address())
            .ok_or_else(|| Error::ValidatorNotFound(signed_vote.validator_address().clone()))?;

        if !self
            .ctx
            .verify_signed_vote(&signed_vote, validator.public_key())
        {
            return Err(Error::InvalidVoteSignature(
                signed_vote.clone(),
                validator.clone(),
            ));
        }

        let round = signed_vote.vote.round();

        let Some(vote_msg) = self
            .votes
            .apply_vote(signed_vote.vote, validator.voting_power())
        else {
            return Ok(None);
        };

        let round_event = match vote_msg {
            VoteMessage::PolkaAny => RoundEvent::PolkaAny,
            VoteMessage::PolkaNil => RoundEvent::PolkaNil,
            VoteMessage::PolkaValue(v) => RoundEvent::PolkaValue(v),
            VoteMessage::PrecommitAny => RoundEvent::PrecommitAny,
            VoteMessage::PrecommitValue(v) => RoundEvent::PrecommitValue(v),
            VoteMessage::SkipRound(r) => RoundEvent::SkipRound(r),
        };

        Ok(self.apply_event(round, round_event))
    }

    fn apply_timeout(&mut self, timeout: Timeout) -> Option<RoundMessage<Ctx>> {
        let event = match timeout.step {
            TimeoutStep::Propose => RoundEvent::TimeoutPropose,
            TimeoutStep::Prevote => RoundEvent::TimeoutPrevote,
            TimeoutStep::Precommit => RoundEvent::TimeoutPrecommit,
        };

        self.apply_event(timeout.round, event)
    }

    /// Apply the event, update the state.
    fn apply_event(&mut self, round: Round, event: RoundEvent<Ctx>) -> Option<RoundMessage<Ctx>> {
        // Get the round state, or create a new one
        let round_state = self.round_states.remove(&round).unwrap_or_default();

        let data = RoundData::new(round, &self.height, &self.address);

        // Multiplex the event with the round state.
        let mux_event = match event {
            RoundEvent::PolkaValue(value_id) => match round_state.proposal {
                Some(ref proposal) if proposal.value().id() == value_id => {
                    RoundEvent::ProposalAndPolkaCurrent(proposal.clone())
                }
                _ => RoundEvent::PolkaAny,
            },
            RoundEvent::PrecommitValue(value_id) => match round_state.proposal {
                Some(ref proposal) if proposal.value().id() == value_id => {
                    RoundEvent::ProposalAndPrecommitValue(proposal.clone())
                }
                _ => RoundEvent::PrecommitAny,
            },

            _ => event,
        };

        // Apply the event to the round state machine
        let transition = round_state.apply_event(&data, mux_event);

        // Update state
        self.round_states.insert(round, transition.next_state);

        // Return message, if any
        transition.message
    }

    pub fn round_state(&self, round: Round) -> Option<&RoundState<Ctx>> {
        self.round_states.get(&round)
    }
}
