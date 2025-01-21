use core::fmt;

use derive_where::derive_where;
use tokio::sync::broadcast;

use malachitebft_core_consensus::{LocallyProposedValue, ProposedValue, SignedConsensusMsg};
use malachitebft_core_types::{CommitCertificate, Context, Round, Timeout, ValueOrigin};

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
    ProposedValue(LocallyProposedValue<Ctx>),
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),
    Decided(CommitCertificate<Ctx>),
    RequestedVoteSet(Ctx::Height, Round),
    SentVoteSetResponse(Ctx::Height, Round, usize),
    WalReplayBegin(Ctx::Height, usize),
    WalReplayConsensus(SignedConsensusMsg<Ctx>),
    WalReplayTimeout(Timeout),
    WalReplayDone(Ctx::Height),
}

impl<Ctx: Context> fmt::Display for Event<Ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::StartedHeight(height) => write!(f, "StartedHeight(height: {height})"),
            Event::StartedRound(height, round) => {
                write!(f, "StartedRound(height: {height}, round: {round})")
            }
            Event::Published(msg) => write!(f, "Published(msg: {msg:?})"),
            Event::ProposedValue(value) => write!(f, "ProposedValue(value: {value:?})"),
            Event::ReceivedProposedValue(value, origin) => {
                write!(
                    f,
                    "ReceivedProposedValue(value: {value:?}, origin: {origin:?})"
                )
            }
            Event::Decided(cert) => write!(f, "Decided(value: {})", cert.value_id),
            Event::RequestedVoteSet(height, round) => {
                write!(f, "RequestedVoteSet(height: {height}, round: {round})")
            }
            Event::SentVoteSetResponse(height, round, count) => {
                write!(
                    f,
                    "SentVoteSetResponse(height: {height}, round: {round}, count: {count})"
                )
            }
            Event::WalReplayBegin(height, count) => {
                write!(f, "WalReplayBegin(height: {height}, count: {count})")
            }
            Event::WalReplayConsensus(msg) => write!(f, "WalReplayConsensus(msg: {msg:?})"),
            Event::WalReplayTimeout(timeout) => write!(f, "WalReplayTimeout(timeout: {timeout:?})"),
            Event::WalReplayDone(height) => write!(f, "WalReplayDone(height: {height})"),
        }
    }
}
