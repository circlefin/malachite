use std::ops::ControlFlow;
use std::thread::JoinHandle;
use std::{io, thread};

use eyre::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

use malachite_core_types::{Context, Height};
use malachite_wal as wal;

use super::entry::{WalCodec, WalEntry};

pub type ReplyTo<T> = oneshot::Sender<Result<T>>;

pub enum WalMsg<Ctx: Context> {
    StartedHeight(Ctx::Height, ReplyTo<Vec<WalEntry<Ctx>>>),
    Append(WalEntry<Ctx>, ReplyTo<()>),
    Flush(ReplyTo<()>),
    Shutdown,
}

pub fn spawn<Ctx, Codec>(
    span: tracing::Span,
    mut log: wal::Log,
    codec: Codec,
    mut rx: mpsc::Receiver<WalMsg<Ctx>>,
) -> JoinHandle<()>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    thread::spawn(move || {
        while let Some(msg) = rx.blocking_recv() {
            match process_msg(msg, &span, &mut log, &codec) {
                Ok(ControlFlow::Continue(())) => continue,
                Ok(ControlFlow::Break(())) => break,
                Err(e) => error!("WAL task failed: {e}"),
            }
        }

        // Task finished normally, stop the thread
        drop(log);
    })
}

#[tracing::instrument(name = "wal", parent = span, skip_all, fields(height = log.sequence()))]
fn process_msg<Ctx, Codec>(
    msg: WalMsg<Ctx>,
    span: &tracing::Span,
    log: &mut wal::Log,
    codec: &Codec,
) -> Result<ControlFlow<()>>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    match msg {
        WalMsg::StartedHeight(height, reply) => {
            // FIXME: Ensure this works even with fork_id
            let sequence = height.as_u64();

            if sequence == log.sequence() {
                // WAL is already at that sequence
                // Let's check if there are any entries to replay
                let entries = fetch_entries(log, codec);

                if reply.send(entries).is_err() {
                    error!("Failed to send WAL replay reply");
                }
            } else {
                // WAL is at different sequence, restart it
                // No entries to replay
                let result = log
                    .restart(sequence)
                    .map(|_| Vec::new())
                    .map_err(Into::into);

                debug!(%height, "Reset WAL");

                if reply.send(result).is_err() {
                    error!("Failed to send WAL reset reply");
                }
            }
        }

        WalMsg::Append(entry, reply) => {
            let tpe = entry.tpe();

            let mut buf = Vec::new();
            entry.encode(codec, &mut buf)?;

            let result = log.append(&buf).map_err(Into::into);

            if let Err(e) = &result {
                error!("ATTENTION: Failed to append entry to WAL: {e}");
            } else {
                debug!(
                    type = %tpe, entry.size = %buf.len(), log.entries = %log.len(),
                    "Wrote log entry"
                );
            }

            if reply.send(result).is_err() {
                error!("Failed to send WAL append reply");
            }
        }

        WalMsg::Flush(reply) => {
            let result = log.flush().map_err(Into::into);

            if let Err(e) = &result {
                error!("ATTENTION: Failed to flush WAL to disk: {e}");
            } else {
                debug!(
                    log.entries = %log.len(),
                    log.size = %log.size_bytes().unwrap_or(0),
                    "Flushed WAL to disk"
                );
            }

            if reply.send(result).is_err() {
                error!("Failed to send WAL flush reply");
            }
        }

        WalMsg::Shutdown => {
            info!("Shutting down WAL thread");
            return Ok(ControlFlow::Break(()));
        }
    }

    Ok(ControlFlow::Continue(()))
}

fn fetch_entries<Ctx, Codec>(log: &mut wal::Log, codec: &Codec) -> Result<Vec<WalEntry<Ctx>>>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    if log.is_empty() {
        return Ok(Vec::new());
    }

    let entries = log
        .iter()?
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(e) => {
                error!("Failed to retrieve a WAL entry: {e}");
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
