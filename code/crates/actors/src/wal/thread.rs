use std::thread::JoinHandle;
use std::{io, thread};

use eyre::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, warn};

use malachite_common::{Context, Height};
use malachite_consensus::SignedConsensusMsg;
use malachite_wal as wal;

use crate::util::codec::NetworkCodec;

use super::entry::WalEntry;

pub type ReplyTo<T> = oneshot::Sender<Result<T>>;

pub enum WalMsg<Ctx: Context> {
    StartedHeight(Ctx::Height, ReplyTo<Vec<WalEntry<Ctx>>>),
    Append(WalEntry<Ctx>, ReplyTo<()>),
    Sync(ReplyTo<()>),
    Shutdown,
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
            // Task failed, log the error and continue
            error!("WAL error: {e}");

            continue;
        }

        // Task finished normally, stop the thread
        break;
    })
}

fn task<Ctx, Codec>(
    log: &mut wal::Log,
    codec: &Codec,
    rx: &mut mpsc::Receiver<WalMsg<Ctx>>,
) -> Result<()>
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
                    // WAL is already at that sequence
                    // Let's check if there are any entries to replay
                    let entries = fetch_entries(log, codec);
                    reply.send(entries).unwrap(); // FIXME
                } else {
                    // WAL is at different sequence, restart it
                    // No entries to replay
                    let result = log
                        .restart(sequence)
                        .map(|_| Vec::new())
                        .map_err(Into::into);

                    reply.send(result).unwrap(); // FIXME
                }
            }

            WalMsg::Append(entry, reply) => {
                let tpe = entry.tpe();

                let mut buf = Vec::new();
                entry.encode(codec, &mut buf)?;

                let result = log.write(&buf).map_err(Into::into);

                if let Err(e) = &result {
                    error!("ATTENTION: Failed to write entry to WAL: {e}");
                }

                if reply.send(result).is_err() {
                    error!("ATTENTION: Failed to send WAL write reply");
                }

                debug!("Wrote log entry: type = {tpe}, log size = {}", log.len());
            }

            WalMsg::Sync(reply) => {
                let result = log.sync().map_err(Into::into);
                reply.send(result).unwrap(); // FIXME

                debug!("Flushed WAL to disk");
            }

            WalMsg::Shutdown => {
                break;
            }
        }
    }

    Ok(())
}

fn fetch_entries<Ctx, Codec>(log: &mut wal::Log, codec: &Codec) -> Result<Vec<WalEntry<Ctx>>>
where
    Ctx: Context,
    Codec: NetworkCodec<SignedConsensusMsg<Ctx>>,
{
    if log.is_empty() {
        return Ok(Vec::new());
    }

    let entries = log
        .iter()?
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(e) => {
                warn!("Failed to retrieve a WAL entry: {e}");
                None
            }
        })
        .filter_map(
            |bytes| match WalEntry::decode(codec, io::Cursor::new(bytes)) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    error!("Failed to decode WAL entry: {e}");
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    Ok(entries)
}
