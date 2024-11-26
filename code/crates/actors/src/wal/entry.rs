use std::io::{self, Read, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use derive_where::derive_where;

use malachite_common::{Context, Round, Timeout};
use malachite_consensus::SignedConsensusMsg;

use crate::util::codec::NetworkCodec;

#[derive_where(Debug)]
pub enum WalEntry<Ctx: Context> {
    ConsensusMsg(SignedConsensusMsg<Ctx>),
    Timeout(Timeout),
}

impl<Ctx> WalEntry<Ctx>
where
    Ctx: Context,
{
    pub fn tpe(&self) -> &'static str {
        match self {
            WalEntry::ConsensusMsg(msg) => match msg {
                SignedConsensusMsg::Vote(_) => "Consensus(Vote)",
                SignedConsensusMsg::Proposal(_) => "Consensus(Proposal)",
            },
            WalEntry::Timeout(_) => "Timeout",
        }
    }
}

impl<Ctx> From<SignedConsensusMsg<Ctx>> for WalEntry<Ctx>
where
    Ctx: Context,
{
    fn from(msg: SignedConsensusMsg<Ctx>) -> Self {
        WalEntry::ConsensusMsg(msg)
    }
}

impl<Ctx> From<Timeout> for WalEntry<Ctx>
where
    Ctx: Context,
{
    fn from(timeout: Timeout) -> Self {
        WalEntry::Timeout(timeout)
    }
}

impl<Ctx> WalEntry<Ctx>
where
    Ctx: Context,
{
    const TAG_CONSENSUS: u8 = 0;
    const TAG_TIMEOUT: u8 = 1;

    pub fn encode<C, W>(&self, codec: &C, mut buf: W) -> io::Result<()>
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

    pub fn decode<C, R>(codec: &C, mut buf: R) -> io::Result<WalEntry<Ctx>>
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

                Ok(WalEntry::ConsensusMsg(msg))
            }

            Self::TAG_TIMEOUT => {
                let timeout = decode_timeout(&mut buf)?;
                Ok(WalEntry::Timeout(timeout))
            }

            _ => Err(io::Error::new(io::ErrorKind::InvalidData, "invalid tag")),
        }
    }
}

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
