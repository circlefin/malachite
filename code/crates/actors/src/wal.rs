use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;

use eyre::eyre;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort, SpawnErr};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, trace};

use malachite_common::{Context, Timeout};
use malachite_consensus::SignedConsensusMsg;
use malachite_metrics::SharedRegistry;
use malachite_wal as wal;

use crate::util::codec::NetworkCodec;

mod entry;
mod thread;

use entry::WalEntry;

pub type WalRef<Ctx> = ActorRef<Msg<Ctx>>;

pub struct Wal<Ctx, Codec> {
    _marker: PhantomData<(Ctx, Codec)>,
}

impl<Ctx, Codec> Wal<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    pub fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }

    pub async fn spawn(
        _ctx: &Ctx,
        codec: Codec,
        path: PathBuf,
        _metrics: SharedRegistry,
    ) -> Result<WalRef<Ctx>, SpawnErr> {
        let (actor_ref, _) = Actor::spawn(None, Self::new(), Args { path, codec }).await?;
        Ok(actor_ref)
    }
}

pub type WalReply = RpcReplyPort<Result<(), io::Error>>;

pub enum Msg<Ctx: Context> {
    StartedHeight(Ctx::Height),
    WriteMsg(SignedConsensusMsg<Ctx>, WalReply),
    WriteTimeout(Ctx::Height, Timeout, WalReply),
    Sync(WalReply),
}

pub struct Args<Codec> {
    pub path: PathBuf,
    pub codec: Codec,
}

pub struct State<Ctx: Context> {
    height: Ctx::Height,
    wal_sender: mpsc::Sender<self::thread::WalMsg<Ctx>>,
    handle: std::thread::JoinHandle<()>,
}

impl<Ctx, Codec> Wal<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    async fn handle_msg(
        &self,
        _myself: WalRef<Ctx>,
        msg: Msg<Ctx>,
        state: &mut State<Ctx>,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            // TODO: Add reply logic?
            Msg::StartedHeight(height) => {
                if state.height == height {
                    debug!(%height, "WAL already at height, ignoring");
                    return Ok(());
                }

                state.height = height;

                self.started_height(state, height).await?;
            }

            Msg::WriteMsg(msg, reply_to) => {
                if msg.msg_height() != state.height {
                    debug!(
                        "Ignoring message with height {} != {}",
                        msg.msg_height(),
                        state.height
                    );

                    return Ok(());
                }

                self.write_log(state, msg, reply_to).await?;
            }

            Msg::WriteTimeout(height, timeout, reply_to) => {
                if height != state.height {
                    debug!(
                        "Ignoring timeout with height {} != {}",
                        height, state.height
                    );

                    return Ok(());
                }

                self.write_log(state, timeout, reply_to).await?;
            }

            Msg::Sync(reply_to) => {
                self.sync_log(state, reply_to).await?;
            }
        }

        Ok(())
    }

    async fn started_height(
        &self,
        state: &mut State<Ctx>,
        height: <Ctx as Context>::Height,
    ) -> Result<(), ActorProcessingErr> {
        let (tx, rx) = oneshot::channel();

        state
            .wal_sender
            .send(self::thread::WalMsg::StartedHeight(height, tx))
            .await?;

        // FIXME: Send
        let _ = rx.await?;

        Ok(())
    }

    async fn write_log(
        &self,
        state: &mut State<Ctx>,
        msg: impl Into<WalEntry<Ctx>>,
        reply_to: WalReply,
    ) -> Result<(), ActorProcessingErr> {
        let entry = msg.into();
        let (tx, rx) = oneshot::channel();

        state
            .wal_sender
            .send(self::thread::WalMsg::Append(entry, tx))
            .await?;

        let result = rx.await?;

        reply_to
            .send(result)
            .map_err(|e| eyre!("Failed to send reply: {e}"))?;

        Ok(())
    }

    async fn sync_log(
        &self,
        state: &mut State<Ctx>,
        reply_to: WalReply,
    ) -> Result<(), ActorProcessingErr> {
        let (tx, rx) = oneshot::channel();

        state
            .wal_sender
            .send(self::thread::WalMsg::Sync(tx))
            .await?;

        let result = rx.await?;

        reply_to
            .send(result)
            .map_err(|e| eyre!("Failed to send reply: {e}"))?;

        Ok(())
    }
}

#[async_trait]
impl<Ctx, Codec> Actor for Wal<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    type Msg = Msg<Ctx>;
    type Arguments = Args<Codec>;
    type State = State<Ctx>;

    async fn pre_start(
        &self,
        _myself: WalRef<Ctx>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let log = wal::Log::open(&args.path)?;
        let (tx, rx) = mpsc::channel(100);
        let handle = self::thread::spawn(log, args.codec, rx);

        Ok(State {
            height: Ctx::Height::default(),
            wal_sender: tx,
            handle,
        })
    }

    async fn handle(
        &self,
        myself: WalRef<Ctx>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        if let Err(e) = self.handle_msg(myself, msg, state).await {
            error!("Failed to handle WAL message: {e}");
        }

        Ok(())
    }
}
