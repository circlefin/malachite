use std::io::{self, Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, BE};

use malachitebft_codec::Codec;
use malachitebft_core_consensus::{Input, ProposedValue, SignedConsensusMsg};
use malachitebft_core_types::{Context, PolkaCertificate, Round, Timeout, ValueOrigin};

/// Codec bounds required to encode and decode WAL entries.
///
/// Automatically implemented for any codec that can encode/decode each of the underlying
/// per-payload types stored in the WAL.
pub trait WalCodec<Ctx>
where
    Ctx: Context,
    Self: Codec<SignedConsensusMsg<Ctx>>,
    Self: Codec<ProposedValue<Ctx>>,
    Self: Codec<PolkaCertificate<Ctx>>,
{
}

impl<Ctx, C> WalCodec<Ctx> for C
where
    Ctx: Context,
    C: Codec<SignedConsensusMsg<Ctx>>,
    C: Codec<ProposedValue<Ctx>>,
    C: Codec<PolkaCertificate<Ctx>>,
{
}

// On-disk tag scheme. Tags are stable: changing them breaks WAL backward compatibility.
//
// `Vote` and `Proposal` share tag 0x01 because both encode through `SignedConsensusMsg<Ctx>`,
// whose Codec implementation handles both variants in a single byte stream.
const TAG_CONSENSUS: u8 = 0x01;
const TAG_TIMEOUT: u8 = 0x02;
const TAG_PROPOSED_VALUE: u8 = 0x04;
const TAG_POLKA_CERTIFICATE: u8 = 0x08;

/// Encode an [`Input`] for the Write-Ahead Log.
///
/// Only inputs whose effects on the driver's equivocation guards must survive a crash are
/// supported. Variants that should not be persisted are rejected with an `InvalidInput` error
/// rather than silently dropped — the WAL contract forbids partial writes.
pub fn encode_entry<Ctx, C, W>(entry: Input<Ctx>, codec: &C, buf: W) -> io::Result<()>
where
    Ctx: Context,
    C: WalCodec<Ctx>,
    W: Write,
{
    match entry {
        Input::Vote(vote) => {
            encode_codec_payload(TAG_CONSENSUS, &SignedConsensusMsg::Vote(vote), codec, buf)
        }
        Input::Proposal(proposal) => encode_codec_payload(
            TAG_CONSENSUS,
            &SignedConsensusMsg::Proposal(proposal),
            codec,
            buf,
        ),
        Input::TimeoutElapsed(timeout) => encode_timeout(TAG_TIMEOUT, &timeout, buf),
        // The `ValueOrigin` tag is intentionally not persisted: replay always re-emerges from
        // local storage, so `Consensus` is the truthful origin on decode and no driver code
        // branches on the tag during replay.
        Input::ProposedValue(value, _origin) => {
            encode_codec_payload(TAG_PROPOSED_VALUE, &value, codec, buf)
        }
        Input::PolkaCertificate(certificate) => {
            encode_codec_payload(TAG_POLKA_CERTIFICATE, &certificate, codec, buf)
        }
        Input::StartHeight(..)
        | Input::Propose(..)
        | Input::RoundCertificate(..)
        | Input::SyncValueResponse(..) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "input variant is not persisted to the WAL",
        )),
    }
}

/// Decode a [`Input`] previously encoded by [`encode_entry`].
///
/// Only the persistable subset of [`Input`] is producible: decoding always yields one of
/// `Vote`, `Proposal`, `TimeoutElapsed`, `ProposedValue`, or `PolkaCertificate`.
pub fn decode_entry<Ctx, C, R>(codec: &C, mut buf: R) -> io::Result<Input<Ctx>>
where
    Ctx: Context,
    C: WalCodec<Ctx>,
    R: Read,
{
    let tag = buf.read_u8()?;

    match tag {
        TAG_CONSENSUS => {
            let msg: SignedConsensusMsg<Ctx> = decode_codec_payload(codec, buf)?;
            Ok(match msg {
                SignedConsensusMsg::Vote(vote) => Input::Vote(vote),
                SignedConsensusMsg::Proposal(proposal) => Input::Proposal(proposal),
            })
        }
        TAG_TIMEOUT => decode_timeout(buf).map(Input::TimeoutElapsed),
        TAG_PROPOSED_VALUE => {
            let value: ProposedValue<Ctx> = decode_codec_payload(codec, buf)?;
            Ok(Input::ProposedValue(value, ValueOrigin::Consensus))
        }
        TAG_POLKA_CERTIFICATE => decode_codec_payload(codec, buf).map(Input::PolkaCertificate),
        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid tag")),
    }
}

fn encode_codec_payload<T, C, W>(tag: u8, value: &T, codec: &C, mut buf: W) -> io::Result<()>
where
    C: Codec<T>,
    W: Write,
{
    let bytes = codec.encode(value).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to encode WAL entry payload: {e}"),
        )
    })?;

    buf.write_u8(tag)?;
    buf.write_u64::<BE>(bytes.len() as u64)?;
    buf.write_all(&bytes)?;

    Ok(())
}

fn decode_codec_payload<T, C, R>(codec: &C, mut buf: R) -> io::Result<T>
where
    C: Codec<T>,
    R: Read,
{
    let len = buf.read_u64::<BE>()?;
    let mut bytes = vec![0; len as usize];
    buf.read_exact(&mut bytes)?;

    codec.decode(bytes.into()).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to decode WAL entry payload: {e}"),
        )
    })
}

// Timeouts use a compact custom format rather than the codec layer:
// [tag: u8] [step: u8] [round: i64 BE].
fn encode_timeout(tag: u8, timeout: &Timeout, mut buf: impl Write) -> io::Result<()> {
    use malachitebft_core_types::TimeoutKind;

    let step = match timeout.kind {
        TimeoutKind::Propose => 1,
        TimeoutKind::Prevote => 2,
        TimeoutKind::Precommit => 3,

        // NOTE: Commit, prevote and precommit time limit timeouts have been removed.

        // Consensus will typically not want to store these timeouts in the WAL,
        // but we still need to handle them here.
        TimeoutKind::Rebroadcast => 7,
        TimeoutKind::FinalizeHeight(_) => {
            // FinalizeHeight timeouts are not persisted to WAL.
            // `InvalidInput` matches the rejection kind used for other non-persistable variants
            // in `encode_entry`, keeping "should not be persisted" distinguishable from
            // genuine encoder bugs (`InvalidData`).
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FinalizeHeight timeout should not be written to WAL",
            ));
        }
    };

    buf.write_u8(tag)?;
    buf.write_u8(step)?;
    buf.write_i64::<BE>(timeout.round.as_i64())?;

    Ok(())
}

fn decode_timeout(mut buf: impl Read) -> io::Result<Timeout> {
    use malachitebft_core_types::TimeoutKind;

    let step = match buf.read_u8()? {
        1 => TimeoutKind::Propose,
        2 => TimeoutKind::Prevote,
        3 => TimeoutKind::Precommit,

        // Commit timeouts have been removed in PR #976,
        // but we still need to handle them here in order to decode old WAL entries.
        4 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "commit timeouts are no longer supported, ignoring",
            ))
        }

        // Prevote/precommit rebroadcast timeouts have been removed in PR #1037,
        // but we still need to handle them here in order to decode old WAL entries.
        5 | 6 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "prevote/precommit time limit timeouts are no longer supported, ignoring",
            ))
        }

        // Consensus will typically not want to store these timeouts in the WAL,
        // but we still need to handle them here.
        7 => TimeoutKind::Rebroadcast,

        // FinalizeHeight timeouts were never actually persisted
        8 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "FinalizeHeight timeouts are not persisted to WAL, ignoring",
            ))
        }

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
