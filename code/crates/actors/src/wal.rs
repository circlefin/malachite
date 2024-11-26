use std::borrow::Cow;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use derive_where::derive_where;
use malachite_metrics::SharedRegistry;
use ractor::{async_trait, Actor, ActorProcessingErr, ActorRef, RpcReplyPort, SpawnErr};
use tokio::task::spawn_blocking;
use tracing::{debug, error};

use malachite_common::{Context, Height, Round, Timeout};
use malachite_consensus::SignedConsensusMsg;
use malachite_wal as wal;

use crate::util::codec::NetworkCodec;

fn encode_timeout(timeout: &Timeout, mut buf: impl Write) -> io::Result<()> {
    use malachite_common::TimeoutStep;

    let step = match timeout.step {
        TimeoutStep::Propose => 1,
        TimeoutStep::Prevote => 2,
        TimeoutStep::Precommit => 3,
        TimeoutStep::Commit => 4,
    };

    buf.write_u8(step)?;
    buf.write_i64::<BE>(timeout.round.as_i64())?;

    Ok(())
}

fn decode_timeout(mut buf: impl Read) -> io::Result<Timeout> {
    use malachite_common::TimeoutStep;

    let step = match buf.read_u8()? {
        1 => TimeoutStep::Propose,
        2 => TimeoutStep::Prevote,
        3 => TimeoutStep::Precommit,
        4 => TimeoutStep::Commit,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid timeout step",
            ))
        }
    };

    let round = Round::from(buf.read_i64::<BE>()?);

    Ok(Timeout::new(round, step))
}

pub type WalRef<Ctx> = ActorRef<Msg<Ctx>>;

pub struct Wal<Ctx, Codec> {
    codec: Codec,
    _marker: PhantomData<Ctx>,
}

impl<Ctx, Codec> Wal<Ctx, Codec>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            _marker: PhantomData,
        }
    }

    pub async fn spawn(
        _ctx: &Ctx,
        codec: Codec,
        path: PathBuf,
        _metrics: SharedRegistry,
    ) -> Result<WalRef<Ctx>, SpawnErr> {
        let (actor_ref, _) = Actor::spawn(None, Self::new(codec), Args { path }).await?;
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

pub struct Args {
    pub path: PathBuf,
}

pub struct State<Ctx: Context> {
    height: Ctx::Height,
    log: Arc<RwLock<wal::Log>>,
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
                state.height = height;

                // FIXME: Ensure this works event with fork_id
                let sequence = height.as_u64();

                let log = Arc::clone(&state.log);
                let result = spawn_blocking(move || log.write().unwrap().restart(sequence)).await?;

                if let Err(e) = &result {
                    error!(%height, "ATTENTION: Failed to restart WAL: {e}");
                }
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

    async fn write_log(
        &self,
        state: &mut State<Ctx>,
        msg: impl Into<WalEntry<'_, Ctx>>,
        reply_to: WalReply,
    ) -> io::Result<()> {
        let entry = msg.into();
        let tpe = entry.tpe();

        let mut buf = Vec::new();
        entry.encode(&self.codec, &mut buf)?;

        let log = Arc::clone(&state.log);
        let result = spawn_blocking(move || log.write().unwrap().write(&buf)).await?;

        if let Err(e) = &result {
            error!("ATTENTION: Failed to write entry to WAL: {e}");
        }

        if let Err(e) = reply_to.send(result) {
            error!("ATTENTION: Failed to send WAL write reply: {e}");
        }

        debug!(
            "Wrote log entry: type = {tpe}, log size = {}",
            state.log.read().unwrap().len()
        );

        Ok(())
    }

    async fn sync_log(&self, state: &mut State<Ctx>, reply_to: WalReply) -> io::Result<()> {
        let log = Arc::clone(&state.log);
        let result = spawn_blocking(move || log.write().unwrap().sync()).await?;

        if let Err(e) = &result {
            error!("ATTENTION: Failed to sync WAL: {e}");
        }

        if let Err(e) = reply_to.send(result) {
            error!("ATTENTION: Failed to send WAL sync reply: {e}");
        }

        debug!("Flushed WAL to disk");

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
    type Arguments = Args;
    type State = State<Ctx>;

    async fn pre_start(
        &self,
        _myself: WalRef<Ctx>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let log = wal::Log::open(&args.path)?;

        Ok(State {
            height: Ctx::Height::default(),
            log: Arc::new(RwLock::new(log)),
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

#[derive_where(Debug)]
enum WalEntry<'a, Ctx: Context> {
    ConsensusMsg(Cow<'a, SignedConsensusMsg<Ctx>>),
    Timeout(Timeout),
}

impl<'a, Ctx> WalEntry<'a, Ctx>
where
    Ctx: Context,
{
    fn tpe(&self) -> &'static str {
        match self {
            WalEntry::ConsensusMsg(msg) => match msg.as_ref() {
                SignedConsensusMsg::Vote(_) => "Consensus(Vote)",
                SignedConsensusMsg::Proposal(_) => "Consensus(Proposal)",
            },
            WalEntry::Timeout(_) => "Timeout",
        }
    }
}

impl<'a, Ctx> From<SignedConsensusMsg<Ctx>> for WalEntry<'a, Ctx>
where
    Ctx: Context,
{
    fn from(msg: SignedConsensusMsg<Ctx>) -> Self {
        WalEntry::ConsensusMsg(Cow::Owned(msg))
    }
}

impl<'a, Ctx> From<&'a SignedConsensusMsg<Ctx>> for WalEntry<'a, Ctx>
where
    Ctx: Context,
{
    fn from(msg: &'a SignedConsensusMsg<Ctx>) -> Self {
        WalEntry::ConsensusMsg(Cow::Borrowed(msg))
    }
}

impl<'a, Ctx> From<Timeout> for WalEntry<'a, Ctx>
where
    Ctx: Context,
{
    fn from(timeout: Timeout) -> Self {
        WalEntry::Timeout(timeout)
    }
}

impl<'a, Ctx> WalEntry<'a, Ctx>
where
    Ctx: Context,
{
    const TAG_CONSENSUS: u8 = 0;
    const TAG_TIMEOUT: u8 = 1;

    fn encode<C, W>(&self, codec: &C, mut buf: W) -> io::Result<()>
    where
        C: NetworkCodec<SignedConsensusMsg<Ctx>>,
        W: Write,
    {
        match self {
            WalEntry::ConsensusMsg(msg) => {
                // Write tag
                buf.write_u8(Self::TAG_CONSENSUS)?;

                let bytes = codec.encode(msg).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to encode msg: {e}"),
                    )
                })?;

                // Write encoded length
                buf.write_u64::<BE>(bytes.len() as u64)?;

                // Write encoded bytes
                buf.write_all(&bytes)?;

                Ok(())
            }

            WalEntry::Timeout(timeout) => {
                // Write tag
                buf.write_u8(Self::TAG_TIMEOUT)?;

                // Write timeout
                encode_timeout(timeout, &mut buf)?;

                Ok(())
            }
        }
    }

    fn decode<C, R>(codec: &C, mut buf: R) -> io::Result<WalEntry<'a, Ctx>>
    where
        C: NetworkCodec<SignedConsensusMsg<Ctx>>,
        R: Read,
    {
        let tag = buf.read_u8()?;

        match tag {
            Self::TAG_CONSENSUS => {
                let len = buf.read_u64::<BE>()?;
                let mut bytes = vec![0; len as usize];
                buf.read_exact(&mut bytes)?;

                let msg = codec.decode(bytes.into()).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("failed to decode consensus msg: {e}"),
                    )
                })?;

                Ok(WalEntry::ConsensusMsg(Cow::Owned(msg)))
            }

            Self::TAG_TIMEOUT => {
                let timeout = decode_timeout(&mut buf)?;
                Ok(WalEntry::Timeout(timeout))
            }

            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid tag")),
        }
    }
}
