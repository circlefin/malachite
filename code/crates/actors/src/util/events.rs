use core::fmt;

use derive_where::derive_where;
use tokio::sync::broadcast;

use malachite_common::{CommitCertificate, Context, Round, Timeout, ValueOrigin};
use malachite_consensus::{ProposedValue, SignedConsensusMsg, ValueToPropose};

pub type RxEvent<Ctx> = broadcast::Receiver<Event<Ctx>>;

pub struct TxEvent<Ctx: Context> {
    tx: broadcast::Sender<Event<Ctx>>,
}

impl<Ctx: Context> TxEvent<Ctx> {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event<Ctx>> {
        self.tx.subscribe()
    }

    pub fn send(&self, event: impl FnOnce() -> Event<Ctx>) {
        if self.tx.receiver_count() > 0 {
            let _ = self.tx.send(event());
        }
    }
}

impl<Ctx: Context> Default for TxEvent<Ctx> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive_where(Clone, Debug)]
pub enum Event<Ctx: Context> {
    StartedHeight(Ctx::Height),
    StartedRound(Ctx::Height, Round),
    Published(SignedConsensusMsg<Ctx>),
    ProposedValue(ValueToPropose<Ctx>),
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),
    Decided(CommitCertificate<Ctx>),
    WalReplayBegin(Ctx::Height, usize),
    WalReplayConsensus(SignedConsensusMsg<Ctx>),
    WalReplayTimeout(Timeout),
    WalReplayDone(Ctx::Height),
}

impl<Ctx: Context> fmt::Display for Event<Ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::StartedHeight(height) => write!(f, "StartedHeight({height})"),
            Event::StartedRound(height, round) => write!(f, "StartedRound({height}, {round})"),
            Event::Published(msg) => write!(f, "Published({msg:?})"),
            Event::ProposedValue(value) => write!(f, "ProposedValue({value:?})"),
            Event::ReceivedProposedValue(value, origin) => {
                write!(f, "ReceivedProposedValue({value:?}, {origin:?})")
            }
            Event::Decided(cert) => write!(f, "Decided({cert:?})"),
            Event::WalReplayBegin(height, count) => {
                write!(f, "WalReplayBegin({height}, {count})")
            }
            Event::WalReplayConsensus(msg) => write!(f, "WalReplayConsensus({msg:?})"),
            Event::WalReplayTimeout(timeout) => write!(f, "WalReplayTimeout({timeout:?})"),
            Event::WalReplayDone(height) => write!(f, "WalReplayDone({height})"),
        }
    }
}
