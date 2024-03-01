use std::collections::HashMap;
use std::time::Duration;

use malachite_common::{Timeout, TimeoutStep};
use ractor::time::send_after;
use ractor::{Actor, ActorName, ActorProcessingErr, ActorRef, MessagingErr};
use tokio::task::JoinHandle;

use super::Config;

pub struct TimeoutElapsed(Timeout);

pub struct Timers<M> {
    config: Config,
    listener: ActorRef<M>,
}

impl<M> Timers<M>
where
    M: From<TimeoutElapsed> + ractor::Message,
{
    pub async fn spawn(
        config: Config,
        listener: ActorRef<M>,
    ) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(
            Some(ActorName::from("Timers")),
            Self { config, listener },
            (),
        )
        .await
    }

    pub fn timeout_duration(&self, step: &TimeoutStep) -> Duration {
        match step {
            TimeoutStep::Propose => self.config.propose_timeout,
            TimeoutStep::Prevote => self.config.prevote_timeout,
            TimeoutStep::Precommit => self.config.precommit_timeout,
            TimeoutStep::Commit => self.config.commit_timeout,
        }
    }
}

pub enum Msg {
    ScheduleTimeout(Timeout),
    CancelTimeout(Timeout),

    // Internal messages
    #[doc(hidden)]
    TimeoutElapsed(Timeout),
}

type TimerTask = JoinHandle<Result<(), MessagingErr<Msg>>>;

#[derive(Default)]
pub struct State {
    timers: HashMap<Timeout, TimerTask>,
}

#[ractor::async_trait]
impl<M> Actor for Timers<M>
where
    M: From<TimeoutElapsed> + ractor::Message,
{
    type Msg = Msg;
    type State = State;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Msg>,
        _args: (),
    ) -> Result<State, ActorProcessingErr> {
        Ok(State::default())
    }

    async fn handle(
        &self,
        myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::ScheduleTimeout(timeout) => {
                let duration = self.timeout_duration(&timeout.step);
                let task = send_after(duration, myself.get_cell(), move || {
                    Msg::TimeoutElapsed(timeout)
                });

                state.timers.insert(timeout, task);
            }

            Msg::CancelTimeout(timeout) => {
                if let Some(task) = state.timers.remove(&timeout) {
                    task.abort();
                }
            }

            Msg::TimeoutElapsed(timeout) => {
                state.timers.remove(&timeout);
                self.listener.cast(TimeoutElapsed(timeout).into())?;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Msg>,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        for (_, task) in state.timers.drain() {
            task.abort();
        }

        Ok(())
    }
}
