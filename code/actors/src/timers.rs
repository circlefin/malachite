use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use ractor::time::send_after;
use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, MessagingErr};
use tokio::task::JoinHandle;

use malachite_common::{Timeout, TimeoutStep};

pub use malachite_node::config::TimeoutConfig as Config;

pub struct TimeoutElapsed(Timeout);

impl TimeoutElapsed {
    pub fn timeout(&self) -> Timeout {
        self.0
    }
}

pub type TimersRef = ActorRef<Msg>;

pub struct Timers<M> {
    listener: ActorRef<M>,
    initial_config: Config,
}

impl<M> Timers<M>
where
    M: From<TimeoutElapsed> + ractor::Message,
{
    pub async fn spawn(
        initial_config: Config,
        listener: ActorRef<M>,
    ) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn(
            None,
            Self {
                listener,
                initial_config,
            },
            initial_config,
        )
        .await
    }

    pub async fn spawn_linked(
        initial_config: Config,
        listener: ActorRef<M>,
        supervisor: ActorCell,
    ) -> Result<(ActorRef<Msg>, JoinHandle<()>), ractor::SpawnErr> {
        Actor::spawn_linked(
            None,
            Self {
                listener,
                initial_config,
            },
            initial_config,
            supervisor,
        )
        .await
    }
}

pub enum Msg {
    /// Schedule the given timeout
    ScheduleTimeout(Timeout),

    /// Cancel the given timeout
    CancelTimeout(Timeout),

    /// Cancel all the timeouts
    CancelAllTimeouts,

    /// Reset all timeouts values to their original values
    ResetTimeouts,

    // Internal messages
    #[doc(hidden)]
    TimeoutElapsed(Timeout),
}

type TimerTask = JoinHandle<Result<(), MessagingErr<Msg>>>;

#[derive(Default)]
pub struct State {
    config: Config,
    timers: HashMap<Timeout, TimerTask>,
}

impl State {
    pub fn timeout_elapsed(&mut self, timeout: &Timeout) {
        self.timers.remove(timeout);
        self.increase_timeout(&timeout.step);
    }

    pub fn increase_timeout(&mut self, step: &TimeoutStep) {
        match step {
            TimeoutStep::Propose => {
                self.config.timeout_propose += self.config.timeout_propose_delta
            }
            TimeoutStep::Prevote => {
                self.config.timeout_prevote += self.config.timeout_prevote_delta
            }
            TimeoutStep::Precommit => {
                self.config.timeout_precommit += self.config.timeout_precommit_delta
            }
            TimeoutStep::Commit => (),
        }
    }

    pub fn timeout_duration(&self, step: &TimeoutStep) -> Duration {
        match step {
            TimeoutStep::Propose => self.config.timeout_propose,
            TimeoutStep::Prevote => self.config.timeout_prevote,
            TimeoutStep::Precommit => self.config.timeout_precommit,
            TimeoutStep::Commit => self.config.timeout_commit,
        }
    }
}

#[async_trait]
impl<M> Actor for Timers<M>
where
    M: From<TimeoutElapsed> + ractor::Message,
{
    type Msg = Msg;
    type State = State;
    type Arguments = Config;

    async fn pre_start(
        &self,
        _myself: ActorRef<Msg>,
        config: Config,
    ) -> Result<State, ActorProcessingErr> {
        Ok(State {
            config,
            ..Default::default()
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Msg>,
        msg: Msg,
        state: &mut State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            Msg::ScheduleTimeout(timeout) => {
                let duration = state.timeout_duration(&timeout.step);
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
                state.timeout_elapsed(&timeout);
                self.listener.cast(TimeoutElapsed(timeout).into())?;
            }

            Msg::CancelAllTimeouts => {
                // Cancel all the timers
                for (_, task) in state.timers.drain() {
                    task.abort();
                }
            }

            Msg::ResetTimeouts => {
                // Reset the timeouts to their original values
                state.config = self.initial_config;
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

// #[cfg(test)]
// #[allow(non_upper_case_globals)]
// mod tests {
//     use malachite_common::Round;
//
//     use super::*;
//
//     const config: Config = Config {
//         propose_timeout: Duration::from_millis(50),
//         prevote_timeout: Duration::from_millis(100),
//         precommit_timeout: Duration::from_millis(150),
//         commit_timeout: Duration::from_millis(200),
//     };
//
//     const fn timeouts() -> (Timeout, Timeout, Timeout) {
//         let (r0, r1, r2) = (Round::new(0), Round::new(1), Round::new(2));
//
//         (
//             Timeout::new(r0, TimeoutStep::Propose),
//             Timeout::new(r1, TimeoutStep::Prevote),
//             Timeout::new(r2, TimeoutStep::Precommit),
//         )
//     }
//
//     #[tokio::test]
//     async fn timers_no_cancel() {
//         let (t0, t1, t2) = timeouts();
//
//         let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);
//
//         timers.schedule_timeout(t1).await;
//         timers.schedule_timeout(t0).await;
//         timers.schedule_timeout(t2).await;
//         assert_eq!(timers.scheduled().await, 3);
//
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t0);
//         assert_eq!(timers.scheduled().await, 2);
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t1);
//         assert_eq!(timers.scheduled().await, 1);
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
//         assert_eq!(timers.scheduled().await, 0);
//     }
//
//     #[tokio::test]
//     async fn timers_cancel_first() {
//         let (t0, t1, t2) = timeouts();
//
//         let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);
//
//         timers.schedule_timeout(t0).await;
//         timers.schedule_timeout(t1).await;
//         timers.schedule_timeout(t2).await;
//         assert_eq!(timers.scheduled().await, 3);
//
//         timers.cancel_timeout(&t0).await;
//         assert_eq!(timers.scheduled().await, 2);
//
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t1);
//         assert_eq!(timers.scheduled().await, 1);
//
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
//         assert_eq!(timers.scheduled().await, 0);
//     }
//
//     #[tokio::test]
//     async fn timers_cancel_middle() {
//         let (t0, t1, t2) = timeouts();
//
//         let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);
//
//         timers.schedule_timeout(t2).await;
//         timers.schedule_timeout(t1).await;
//         timers.schedule_timeout(t0).await;
//         assert_eq!(timers.scheduled().await, 3);
//
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t0);
//         assert_eq!(timers.scheduled().await, 2);
//
//         timers.cancel_timeout(&t1).await;
//         assert_eq!(timers.scheduled().await, 1);
//
//         assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
//         assert_eq!(timers.scheduled().await, 0);
//     }
// }
