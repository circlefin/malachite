use std::collections::HashMap;
use std::time::Duration;

use malachite_common::{Timeout, TimeoutStep};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub struct Config {
    pub propose_timeout: Duration,
    pub prevote_timeout: Duration,
    pub precommit_timeout: Duration,
}

pub struct Timers {
    config: Config,
    timeouts: HashMap<Timeout, JoinHandle<()>>,

    timeout_elapsed: mpsc::Sender<Timeout>,
}

impl Timers {
    pub fn new(config: Config) -> (Self, mpsc::Receiver<Timeout>) {
        let (tx_timeout_elapsed, rx_timeout_elapsed) = mpsc::channel(100);

        let timers = Self {
            config,
            timeouts: HashMap::new(),
            timeout_elapsed: tx_timeout_elapsed,
        };

        (timers, rx_timeout_elapsed)
    }

    pub fn reset(&mut self) {
        for (_, handle) in self.timeouts.drain() {
            handle.abort();
        }
    }

    pub async fn schedule_timeout(&mut self, timeout: Timeout) {
        let tx = self.timeout_elapsed.clone();
        let duration = self.timeout_duration(&timeout);

        let handle = tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            tx.send(timeout).await.unwrap();
        });

        self.timeouts.insert(timeout, handle);
    }

    pub async fn cancel_timeout(&mut self, timeout: &Timeout) {
        if let Some(handle) = self.timeouts.remove(timeout) {
            handle.abort();
        }
    }

    fn timeout_duration(&self, timeout: &Timeout) -> Duration {
        match timeout.step {
            TimeoutStep::Propose => self.config.propose_timeout,
            TimeoutStep::Prevote => self.config.prevote_timeout,
            TimeoutStep::Precommit => self.config.precommit_timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use malachite_common::Round;

    use super::*;

    #[tokio::test]
    async fn test_timers() {
        let config = Config {
            propose_timeout: Duration::from_millis(100),
            prevote_timeout: Duration::from_millis(200),
            precommit_timeout: Duration::from_millis(300),
        };

        let (r0, r1, r2) = (Round::new(0), Round::new(1), Round::new(2));
        let (t0, t1, t2) = (
            Timeout::new(r0, TimeoutStep::Propose),
            Timeout::new(r1, TimeoutStep::Prevote),
            Timeout::new(r2, TimeoutStep::Precommit),
        );

        let (mut timers, mut rx_timeout_elapsed) = Timers::new(config);

        timers.schedule_timeout(t2).await;
        timers.schedule_timeout(t1).await;
        timers.schedule_timeout(t0).await;

        assert_eq!(rx_timeout_elapsed.recv().await.unwrap(), t0);

        timers.cancel_timeout(&t1).await;

        assert_eq!(
            rx_timeout_elapsed.recv().await.unwrap(),
            Timeout::new(r2, TimeoutStep::Precommit)
        );
    }
}
