use std::thread::JoinHandle;
use std::{io, thread};

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, trace};

use malachite_common::{Context, Height};
use malachite_consensus::SignedConsensusMsg;
use malachite_wal as wal;

use crate::util::codec::NetworkCodec;

use super::entry::WalEntry;

pub type ReplyTo<T> = oneshot::Sender<Result<T, io::Error>>;

pub enum WalMsg<Ctx: Context> {
    StartedHeight(Ctx::Height, ReplyTo<()>),
    Append(WalEntry<Ctx>, ReplyTo<()>),
    Sync(ReplyTo<()>),
    GetSequence(ReplyTo<u64>),
}

pub fn spawn<Ctx, Codec>(
    mut wal: wal::Log,
    codec: Codec,
    mut rx: mpsc::Receiver<WalMsg<Ctx>>,
) -> JoinHandle<()>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    thread::spawn(move || loop {
        if let Err(e) = task(&mut wal, &codec, &mut rx) {
            error!("Error: {e}");
            continue;
        }

        break;
    })
}

fn task<Ctx, Codec>(
    log: &mut wal::Log,
    codec: &Codec,
    rx: &mut mpsc::Receiver<WalMsg<Ctx>>,
) -> io::Result<()>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            WalMsg::StartedHeight(height, reply) => {
                // FIXME: Ensure this works event with fork_id
                let sequence = height.as_u64();

                if sequence == log.sequence() {
                    trace!(%height, "WAL already at height");
                }

                let result = log.restart(sequence);
                reply.send(result).unwrap(); // FIXME
            }

            WalMsg::Append(entry, reply) => {
                let tpe = entry.tpe();

                let mut buf = Vec::new();
                entry.encode(codec, &mut buf)?;

                let result = log.write(&buf);

                if let Err(e) = &result {
                    error!("ATTENTION: Failed to write entry to WAL: {e}");
                }

                if let Err(_) = reply.send(result) {
                    error!("ATTENTION: Failed to send WAL write reply");
                }

                debug!("Wrote log entry: type = {tpe}, log size = {}", log.len());
            }

            WalMsg::Sync(reply) => {
                let result = log.sync();
                reply.send(result).unwrap(); // FIXME

                debug!("Flushed WAL to disk");
            }

            WalMsg::GetSequence(reply) => {
                let seq = log.sequence();
                reply.send(Ok(seq)).unwrap(); // DIXME
            }
        }
    }

    Ok(())
}
