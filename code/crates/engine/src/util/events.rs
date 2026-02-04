use core::fmt;
use std::io;
use std::sync::Arc;

use derive_where::derive_where;
use tokio::sync::broadcast;

use malachitebft_core_consensus::{
    Error as ConsensusError, LocallyProposedValue, MisbehaviorEvidence, ProposedValue, Role,
    SignedConsensusMsg, WalEntry,
};
use malachitebft_core_types::{
    CommitCertificate, Context, PolkaCertificate, Round, RoundCertificate, SignedVote, ValueOrigin,
};

pub type RxEvent<Ctx> = broadcast::Receiver<Event<Ctx>>;

#[derive_where(Clone)]
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
    StartedHeight(Ctx::Height, bool),
    StartedRound(Ctx::Height, Round, Ctx::Address, Role),
    Published(SignedConsensusMsg<Ctx>),
    Received(SignedConsensusMsg<Ctx>),
    ProposedValue(LocallyProposedValue<Ctx>),
    ReceivedProposedValue(ProposedValue<Ctx>, ValueOrigin),
    Decided {
        commit_certificate: CommitCertificate<Ctx>,
        evidence: MisbehaviorEvidence<Ctx>,
    },
    RepublishVote(SignedVote<Ctx>),
    RebroadcastRoundCertificate(RoundCertificate<Ctx>),
    SkipRoundCertificate(RoundCertificate<Ctx>),
    PolkaCertificate(PolkaCertificate<Ctx>),
    WalReplayBegin(Ctx::Height, usize),
    WalReplayEntry(WalEntry<Ctx>),
    WalReplayDone(Ctx::Height),
    WalReplayError(Arc<ConsensusError<Ctx>>),
    WalResetError(Arc<eyre::Report>),
    WalCorrupted(Arc<io::Error>),
}

impl<Ctx: Context> fmt::Display for Event<Ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::StartedHeight(height, restart) => {
                write!(f, "StartedHeight(height: {height}, restart: {restart})")
            }
            Event::StartedRound(height, round, proposer, role) => {
                write!(f, "StartedRound(height: {height}, round: {round}, proposer: {proposer}, role: {role:?})")
            }
            Event::Published(msg) => write!(f, "Published(msg: {msg:?})"),
            Event::Received(msg) => write!(f, "Received(msg: {msg:?})"),
            Event::ProposedValue(value) => write!(f, "ProposedValue(value: {value:?})"),
            Event::ReceivedProposedValue(value, origin) => {
                write!(
                    f,
                    "ReceivedProposedValue(value: {value:?}, origin: {origin:?})"
                )
            }
            Event::Decided {
                commit_certificate,
                evidence,
            } => {
                if evidence.is_empty() {
                    write!(f, "Decided(value: {})", commit_certificate.value_id)
                } else {
                    write!(
                        f,
                        "Decided(value: {}, evidence: {:?})",
                        commit_certificate.value_id, evidence
                    )
                }
            }
            Event::RepublishVote(vote) => write!(f, "RepublishVote(vote: {vote:?})"),
            Event::RebroadcastRoundCertificate(certificate) => write!(
                f,
                "RebroadcastRoundCertificate(certificate: {certificate:?})"
            ),
            Event::WalReplayBegin(height, count) => {
                write!(f, "WalReplayBegin(height: {height}, count: {count})")
            }
            Event::WalReplayEntry(entry) => write!(f, "WalReplayEntry(entry: {entry:?})"),
            Event::WalReplayDone(height) => write!(f, "WalReplayDone(height: {height})"),
            Event::WalReplayError(error) => write!(f, "WalReplayError({error})"),
            Event::WalResetError(error) => write!(f, "WalResetError({error})"),
            Event::WalCorrupted(error) => write!(f, "WalCorrupted(error: {error:?})"),

            Event::PolkaCertificate(certificate) => {
                write!(f, "PolkaCertificate: {certificate:?})")
            }
            Event::SkipRoundCertificate(certificate) => {
                write!(f, "SkipRoundCertificate: {certificate:?})")
            }
        }
    }
}
