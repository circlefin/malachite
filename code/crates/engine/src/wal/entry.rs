use std::io::{self, Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, BE};

use malachitebft_codec::Codec;
use malachitebft_core_consensus::{ProposedValue, SignedConsensusMsg};
use malachitebft_core_types::{Context, Round, Timeout};

/// Codec for encoding and decoding WAL entries.
///
/// This trait is automatically implemented for any type that implements:
/// - [`Codec<SignedConsensusMsg<Ctx>>`]
pub trait WalCodec<Ctx>
where
    Ctx: Context,
    Self: Codec<SignedConsensusMsg<Ctx>>,
    Self: Codec<ProposedValue<Ctx>>,
{
}

impl<Ctx, C> WalCodec<Ctx> for C
where
    Ctx: Context,
    C: Codec<SignedConsensusMsg<Ctx>>,
    C: Codec<ProposedValue<Ctx>>,
{
}

pub use malachitebft_core_consensus::WalEntry;

const TAG_CONSENSUS: u8 = 0x01;
const TAG_TIMEOUT: u8 = 0x02;
const TAG_PROPOSED_VALUE: u8 = 0x04;

pub fn encode_entry<Ctx, C, W>(entry: &WalEntry<Ctx>, codec: &C, mut buf: W) -> io::Result<()>
where
    Ctx: Context,
    C: WalCodec<Ctx>,
    W: Write,
{
    match entry {
        WalEntry::ConsensusMsg(msg) => {
            let bytes = codec.encode(msg).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to encode consensus message: {e}"),
                )
            })?;

            // Write tag
            buf.write_u8(TAG_CONSENSUS)?;

            // Write encoded length
            buf.write_u64::<BE>(bytes.len() as u64)?;

            // Write encoded bytes
            buf.write_all(&bytes)?;

            Ok(())
        }

        WalEntry::Timeout(timeout) => {
            // Write tag and timeout if applicable
            encode_timeout(TAG_TIMEOUT, timeout, &mut buf)?;

            Ok(())
        }

        WalEntry::ProposedValue(value) => {
            let bytes = codec.encode(value).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to encode consensus message: {e}"),
                )
            })?;

            // Write tag
            buf.write_u8(TAG_PROPOSED_VALUE)?;

            // Write encoded length
            buf.write_u64::<BE>(bytes.len() as u64)?;

            // Write encoded bytes
            buf.write_all(&bytes)?;

            Ok(())
        }
    }
}

pub fn decode_entry<Ctx, C, R>(codec: &C, mut buf: R) -> io::Result<WalEntry<Ctx>>
where
    Ctx: Context,
    C: WalCodec<Ctx>,
    R: Read,
{
    let tag = buf.read_u8()?;

    match tag {
        TAG_CONSENSUS => {
            let len = buf.read_u64::<BE>()?;
            let mut bytes = vec![0; len as usize];
            buf.read_exact(&mut bytes)?;

            let msg = codec.decode(bytes.into()).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to decode consensus msg: {e}"),
                )
            })?;

            Ok(WalEntry::ConsensusMsg(msg))
        }

        TAG_TIMEOUT => {
            let timeout = decode_timeout(&mut buf)?;
            Ok(WalEntry::Timeout(timeout))
        }

        TAG_PROPOSED_VALUE => {
            let len = buf.read_u64::<BE>()?;
            let mut bytes = vec![0; len as usize];
            buf.read_exact(&mut bytes)?;

            let value = codec.decode(bytes.into()).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to decode proposed value: {e}"),
                )
            })?;

            Ok(WalEntry::ProposedValue(value))
        }

        _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid tag")),
    }
}

fn encode_timeout(tag: u8, timeout: &Timeout, mut buf: impl Write) -> io::Result<()> {
    use malachitebft_core_types::TimeoutKind;

    let step = match timeout.kind {
        TimeoutKind::Propose => 1,
        TimeoutKind::Prevote => 2,
        TimeoutKind::Precommit => 3,
        TimeoutKind::Commit => 4,

        // Consensus will typically not want to store these two timeouts in the WAL,
        // but we still need to handle them here.
        TimeoutKind::PrevoteTimeLimit => 5,
        TimeoutKind::PrecommitTimeLimit => 6,
        TimeoutKind::PrevoteRebroadcast => 7,
        TimeoutKind::PrecommitRebroadcast => 8,
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
        4 => TimeoutKind::Commit,
        5 => TimeoutKind::PrevoteTimeLimit,
        6 => TimeoutKind::PrecommitTimeLimit,
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
