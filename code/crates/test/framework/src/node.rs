use std::sync::Arc;
use std::time::Duration;

use eyre::bail;
use tracing::info;

use malachitebft_core_consensus::{LocallyProposedValue, SignedConsensusMsg};
use malachitebft_core_types::{Context, Height, SignedVote, Vote, VoteType, VotingPower};
use malachitebft_engine::util::events::Event;
use malachitebft_test::middleware::{DefaultMiddleware, Middleware};

use crate::Expected;

pub type NodeId = usize;

pub enum Step<Ctx, S>
where
    Ctx: Context,
{
    Crash(Duration),
    ResetDb,
    Restart(Duration),
    WaitUntil(u64),
    WaitUntilRound(u32),
    OnEvent(EventHandler<Ctx, S>),
    Expect(Expected),
    Success,
    Fail(String),
}

#[derive(Copy, Clone, Debug)]
pub enum HandlerResult {
    WaitForNextEvent,
    ContinueTest,
}

pub type EventHandler<Ctx, S> =
    Box<dyn Fn(Event<Ctx>, &mut S) -> Result<HandlerResult, eyre::Report> + Send + Sync>;

pub struct TestNode<Ctx, State = ()>
where
    Ctx: Context,
{
    pub id: NodeId,
    pub voting_power: VotingPower,
    pub start_height: Ctx::Height,
    pub start_delay: Duration,
    pub steps: Vec<Step<Ctx, State>>,
    pub state: State,
    pub middleware: Arc<dyn Middleware>,
}

impl<Ctx, State> TestNode<Ctx, State>
where
    Ctx: Context,
{
    pub fn new(id: usize) -> Self
    where
        State: Default,
    {
        Self::new_with_state(id, State::default())
    }

    pub fn new_with_state(id: usize, state: State) -> Self {
        Self {
            id,
            voting_power: 1,
            start_height: Ctx::Height::INITIAL,
            start_delay: Duration::from_secs(0),
            steps: vec![],
            state,
            middleware: Arc::new(DefaultMiddleware),
        }
    }

    pub fn with_middleware(&mut self, middleware: impl Middleware + 'static) -> &mut Self {
        self.middleware = Arc::new(middleware);
        self
    }

    pub fn with_state(&mut self, state: State) -> &mut Self {
        self.state = state;
        self
    }

    pub fn with_voting_power(&mut self, power: VotingPower) -> &mut Self {
        self.voting_power = power;
        self
    }

    pub fn start(&mut self) -> &mut Self {
        self.start_height = <Ctx::Height>::INITIAL;
        self
    }

    pub fn start_at(&mut self, height: u64) -> &mut Self {
        self.start_after(height, Duration::from_secs(0))
    }

    pub fn start_after(&mut self, height: u64, delay: Duration) -> &mut Self {
        self.start_height = Ctx::Height::ZERO.increment_by(height);
        self.start_delay = delay;
        self
    }

    pub fn crash(&mut self) -> &mut Self {
        self.steps.push(Step::Crash(Duration::from_secs(0)));
        self
    }

    pub fn crash_after(&mut self, duration: Duration) -> &mut Self {
        self.steps.push(Step::Crash(duration));
        self
    }

    pub fn reset_db(&mut self) -> &mut Self {
        self.steps.push(Step::ResetDb);
        self
    }

    pub fn restart_after(&mut self, delay: Duration) -> &mut Self {
        self.steps.push(Step::Restart(delay));
        self
    }

    pub fn wait_until(&mut self, height: u64) -> &mut Self {
        self.steps.push(Step::WaitUntil(height));
        self
    }

    pub fn wait_until_round(&mut self, round: u32) -> &mut Self {
        self.steps.push(Step::WaitUntilRound(round));
        self
    }

    pub fn on_event<F>(&mut self, on_event: F) -> &mut Self
    where
        F: Fn(Event<Ctx>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.steps.push(Step::OnEvent(Box::new(on_event)));
        self
    }

    pub fn expect_wal_replay(&mut self, at_height: u64) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::WalReplayBegin(height, count) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            info!("Replaying WAL at height {height} with {count} messages");

            if height.as_u64() != at_height {
                bail!("Unexpected WAL replay at height {height}, expected {at_height}")
            }

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_vote_set_request(&mut self, at_height: u64) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::RequestedVoteSet(height, round) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            info!("Requested vote set for height {height} and round {round}");

            if height.as_u64() != at_height {
                bail!("Unexpected vote set request for height {height}, expected {at_height}")
            }

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_vote_rebroadcast(
        &mut self,
        at_height: u64,
        at_round: u32,
        vote_type: VoteType,
    ) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::RebroadcastVote(msg) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            let (height, round) = (msg.height(), msg.round());

            if height.as_u64() != at_height {
                bail!("Unexpected vote rebroadcast for height {height}, expected {at_height}")
            }

            if round.as_u32() != Some(at_round) {
                bail!("Unexpected vote rebroadcast for round {round}, expected {at_round}")
            }

            if vote_type != msg.vote_type() {
                bail!(
                    "Unexpected vote type {vote_type:?}, expected {:?}",
                    msg.vote_type()
                )
            }

            info!(%height, %round, ?vote_type, "Rebroadcasted vote");

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_round_certificate_rebroadcast(
        &mut self,
        at_height: u64,
        at_round: u32,
    ) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::RebroadcastRoundCertificate(msg) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            let (height, round) = (msg.height, msg.round);

            if height.as_u64() != at_height {
                bail!("Unexpected round certificate rebroadcast for height {height}, expected {at_height}")
            }

            if round.as_u32() != Some(at_round) {
                bail!("Unexpected round certificate rebroadcast for round {round}, expected {at_round}")
            }

            info!(%height, %round, "Rebroadcasted round certificate");

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_skip_round_certificate(&mut self, at_height: u64, at_round: u32) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::SkipRoundCertificate(msg) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            let (height, round) = (msg.height, msg.round);

            if height.as_u64() != at_height {
                bail!("Unexpected round certificate broadcast for height {height}, expected {at_height}")
            }

            if round.as_u32() != Some(at_round) {
                bail!("Unexpected round certificate broadcast for round {round}, expected {at_round}")
            }

            info!(%height, %round, "Broadcasted skip round certificate");

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn expect_polka_certificate(&mut self, at_height: u64, at_round: u32) -> &mut Self {
        self.on_event(move |event, _| {
            let Event::PolkaCertificate(msg) = event else {
                return Ok(HandlerResult::WaitForNextEvent);
            };

            let (height, round) = (msg.height, msg.round);

            if height.as_u64() != at_height {
                bail!("Unexpected round certificate rebroadcast for height {height}, expected {at_height}")
            }

            if round.as_u32() != Some(at_round) {
                bail!("Unexpected round certificate rebroadcast for round {round}, expected {at_round}")
            }

            info!(%height, %round, "Broadcasted round certificate");

            Ok(HandlerResult::ContinueTest)
        })
    }

    pub fn on_proposed_value<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(LocallyProposedValue<Ctx>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.on_event(move |event, state| {
            if let Event::ProposedValue(value) = event {
                f(value, state)
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
    }

    pub fn on_vote<F>(&mut self, f: F) -> &mut Self
    where
        F: Fn(SignedVote<Ctx>, &mut State) -> Result<HandlerResult, eyre::Report>
            + Send
            + Sync
            + 'static,
    {
        self.on_event(move |event, state| {
            if let Event::Published(SignedConsensusMsg::Vote(vote)) = event {
                f(vote, state)
            } else {
                Ok(HandlerResult::WaitForNextEvent)
            }
        })
    }

    pub fn expect_decisions(&mut self, expected: Expected) -> &mut Self {
        self.steps.push(Step::Expect(expected));
        self
    }

    pub fn success(&mut self) -> &mut Self {
        self.steps.push(Step::Success);
        self
    }

    pub fn full_node(&mut self) -> &mut Self {
        self.voting_power = 0;
        self
    }

    pub fn is_full_node(&self) -> bool {
        self.voting_power == 0
    }

    pub fn with(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        f(self);
        self
    }
}
