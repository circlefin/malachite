use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use malachite_common::{Timeout, TimeoutStep};
use tokio::sync::mpsc;
use tokio::sync::Mutex; // TODO: Use parking_lot instead?
use tokio::task::JoinHandle;

#[derive(Copy, Clone, Debug)]
pub struct Config {
    pub propose_timeout: Duration,
    pub prevote_timeout: Duration,
    pub precommit_timeout: Duration,
    pub commit_timeout: Duration,
}

pub struct Timers {
    config: Config,
    timeouts: Arc<Mutex<HashMap<Timeout, JoinHandle<()>>>>,
    timeout_elapsed: mpsc::Sender<Timeout>,
}

impl Timers {
    pub fn new(config: Config) -> (Self, mpsc::Receiver<Timeout>) {
        let (tx_timeout_elapsed, rx_timeout_elapsed) = mpsc::channel(100);

        let timers = Self {
            config,
            timeouts: Arc::new(Mutex::new(HashMap::new())),
            timeout_elapsed: tx_timeout_elapsed,
        };

        (timers, rx_timeout_elapsed)
    }

    pub async fn reset(&mut self) {
        for (_, handle) in self.timeouts.lock().await.drain() {
            handle.abort();
        }
    }

    pub async fn scheduled(&self) -> usize {
        self.timeouts.lock().await.len()
    }

    pub async fn schedule_timeout(&mut self, timeout: Timeout) {
        let tx = self.timeout_elapsed.clone();
        let duration = self.timeout_duration(&timeout.step);

        let timeouts = self.timeouts.clone();
        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            timeouts.lock().await.remove(&timeout);
            tx.send(timeout).await.unwrap();
        });

        self.timeouts.lock().await.insert(timeout, handle);
    }

    pub async fn cancel_timeout(&mut self, timeout: &Timeout) {
        if let Some(handle) = self.timeouts.lock().await.remove(timeout) {
            handle.abort();
        }
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

#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests {
    use malachite_common::Round;

    use super::*;

    const config: Config = Config {
        propose_timeout: Duration::from_millis(50),
        prevote_timeout: Duration::from_millis(100),
        precommit_timeout: Duration::from_millis(150),
        commit_timeout: Duration::from_millis(200),
    };

    const fn timeouts() -> (Timeout, Timeout, Timeout) {
        let (r0, r1, r2) = (Round::new(0), Round::new(1), Round::new(2));

        (
            Timeout::new(r0, TimeoutStep::Propose),
            Timeout::new(r1, TimeoutStep::Prevote),
            Timeout::new(r2, TimeoutStep::Precommit),
        )
    }

    #[tokio::test]
    async fn timers_no_cancel() {
        let (t0, t1, t2) = timeouts();

        let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);

        timers.schedule_timeout(t1).await;
        timers.schedule_timeout(t0).await;
        timers.schedule_timeout(t2).await;
        assert_eq!(timers.scheduled().await, 3);

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t0);
        assert_eq!(timers.scheduled().await, 2);
        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t1);
        assert_eq!(timers.scheduled().await, 1);
        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
        assert_eq!(timers.scheduled().await, 0);
    }

    #[tokio::test]
    async fn timers_cancel_first() {
        let (t0, t1, t2) = timeouts();

        let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);

        timers.schedule_timeout(t0).await;
        timers.schedule_timeout(t1).await;
        timers.schedule_timeout(t2).await;
        assert_eq!(timers.scheduled().await, 3);

        timers.cancel_timeout(&t0).await;
        assert_eq!(timers.scheduled().await, 2);

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t1);
        assert_eq!(timers.scheduled().await, 1);

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
        assert_eq!(timers.scheduled().await, 0);
    }

    #[tokio::test]
    async fn timers_cancel_middle() {
        let (t0, t1, t2) = timeouts();

        let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);

        timers.schedule_timeout(t2).await;
        timers.schedule_timeout(t1).await;
        timers.schedule_timeout(t0).await;
        assert_eq!(timers.scheduled().await, 3);

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t0);
        assert_eq!(timers.scheduled().await, 2);

        timers.cancel_timeout(&t1).await;
        assert_eq!(timers.scheduled().await, 1);

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t2);
        assert_eq!(timers.scheduled().await, 0);
    }
}
