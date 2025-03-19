use std::ops::ControlFlow;
use std::thread::JoinHandle;
use std::{io, thread};

use eyre::Result;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info};

use malachitebft_core_types::{Context, Height};
use malachitebft_wal as wal;

use super::entry::{WalCodec, WalEntry};
use super::iter::log_entries;

pub type ReplyTo<T> = oneshot::Sender<Result<T>>;

pub enum WalMsg<Ctx: Context> {
    StartedHeight(Ctx::Height, ReplyTo<Vec<WalEntry<Ctx>>>),
    Append(WalEntry<Ctx>, ReplyTo<()>),
    Flush(ReplyTo<()>),
    Shutdown,
    Dump,
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

#[tracing::instrument(
    name = "wal",
    parent = span,
    skip_all,
    fields(height = span_sequence(log.sequence(), &msg))
)]
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

            if !buf.is_empty() {
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
        }

        WalMsg::Flush(reply) => {
            let result = log.flush().map_err(Into::into);

            if let Err(e) = &result {
                error!("ATTENTION: Failed to flush WAL to disk: {e}");
            } else {
                debug!(
                    wal.entries = %log.len(),
                    wal.size = %log.size_bytes().unwrap_or(0),
                    "Flushed WAL to disk"
                );
            }

            if reply.send(result).is_err() {
                error!("Failed to send WAL flush reply");
            }
        }

        WalMsg::Dump => {
            if let Err(e) = dump_entries(log, codec) {
                error!("Failed to dump WAL: {e}");
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
        .enumerate() // Add enumeration to get the index
        .filter_map(|(idx, result)| match result {
            Ok(entry) => Some((idx, entry)),
            Err(e) => {
                error!("Failed to retrieve WAL entry {idx}: {e}");
                None
            }
        })
        .filter_map(
            |(idx, bytes)| match WalEntry::decode(codec, io::Cursor::new(bytes.clone())) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    error!("Failed to decode WAL entry {idx}: {e} {:?}", bytes);
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    if log.len() != entries.len() {
        Err(eyre::eyre!(
            "Failed to fetch and decode all WAL entries: expected {}, got {}",
            log.len(),
            entries.len()
        ))
    } else {
        Ok(entries)
    }
}

fn dump_entries<'a, Ctx, Codec>(log: &'a mut wal::Log, codec: &'a Codec) -> Result<()>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    let len = log.len();
    let mut count = 0;

    info!("WAL Dump");
    info!("- Entries: {len}");
    info!("- Size:    {} bytes", log.size_bytes().unwrap_or(0));
    info!("Entries:");

    for (idx, entry) in log_entries(log, codec)?.enumerate() {
        count += 1;

        match entry {
            Ok(entry) => {
                info!("- #{idx}: {entry:?}");
            }
            Err(e) => {
                error!("- #{idx}: Error decoding WAL entry: {e}");
            }
        }
    }

    if count != len {
        error!("Expected {len} entries, but found {count} entries");
    }

    Ok(())
}

fn span_sequence(sequence: u64, msg: &WalMsg<impl Context>) -> u64 {
    if let WalMsg::StartedHeight(height, _) = msg {
        height.as_u64()
    } else {
        sequence
    }
}
