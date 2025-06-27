use {
    crate::{LivenessMsg, ProposedValue, SignedConsensusMsg},
    ::borsh::{
        io::{Read, Result, Write},
        BorshDeserialize, BorshSerialize,
    },
    malachitebft_core_types::{
        Context, PolkaCertificate, Round, RoundCertificate, SignedProposal, SignedVote, Validity,
    },
};

impl<Ctx: Context> BorshSerialize for SignedConsensusMsg<Ctx>
where
    SignedVote<Ctx>: BorshSerialize,
    SignedProposal<Ctx>: BorshSerialize,
{
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            SignedConsensusMsg::Vote(signed_message) => {
                0u8.serialize(writer)?;
                signed_message.serialize(writer)
            }
            SignedConsensusMsg::Proposal(signed_message) => {
                1u8.serialize(writer)?;
                signed_message.serialize(writer)
            }
        }
    }
}

impl<Ctx: Context> BorshDeserialize for SignedConsensusMsg<Ctx>
where
    SignedVote<Ctx>: BorshDeserialize,
    SignedProposal<Ctx>: BorshDeserialize,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let discriminant = u8::deserialize_reader(reader)?;
        match discriminant {
            0 => Ok(SignedConsensusMsg::Vote(SignedVote::deserialize_reader(
                reader,
            )?)),
            1 => Ok(SignedConsensusMsg::Proposal(
                SignedProposal::deserialize_reader(reader)?,
            )),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid discriminant",
            )),
        }
    }
}

impl<Ctx: Context> BorshSerialize for LivenessMsg<Ctx>
where
    SignedVote<Ctx>: BorshSerialize,
    PolkaCertificate<Ctx>: BorshSerialize,
    RoundCertificate<Ctx>: BorshSerialize,
{
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        match self {
            LivenessMsg::Vote(signed_message) => {
                0u8.serialize(writer)?;
                signed_message.serialize(writer)
            }
            LivenessMsg::PolkaCertificate(polka_certificate) => {
                1u8.serialize(writer)?;
                polka_certificate.serialize(writer)
            }
            LivenessMsg::SkipRoundCertificate(round_certificate) => {
                2u8.serialize(writer)?;
                round_certificate.serialize(writer)
            }
        }
    }
}

impl<Ctx: Context> BorshDeserialize for LivenessMsg<Ctx>
where
    SignedVote<Ctx>: BorshDeserialize,
    PolkaCertificate<Ctx>: BorshDeserialize,
    RoundCertificate<Ctx>: BorshDeserialize,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let discriminant = u8::deserialize_reader(reader)?;
        match discriminant {
            0 => Ok(LivenessMsg::Vote(SignedVote::deserialize_reader(reader)?)),
            1 => Ok(LivenessMsg::PolkaCertificate(
                PolkaCertificate::deserialize_reader(reader)?,
            )),
            2 => Ok(LivenessMsg::SkipRoundCertificate(
                RoundCertificate::deserialize_reader(reader)?,
            )),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid discriminant",
            )),
        }
    }
}

impl<Ctx: Context> BorshSerialize for ProposedValue<Ctx>
where
    Ctx::Height: BorshSerialize,
    Ctx::Address: BorshSerialize,
    Ctx::Value: BorshSerialize,
{
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        self.height.serialize(writer)?;
        self.round.serialize(writer)?;
        self.valid_round.serialize(writer)?;
        self.proposer.serialize(writer)?;
        self.value.serialize(writer)?;
        self.validity.serialize(writer)
    }
}

impl<Ctx: Context> BorshDeserialize for ProposedValue<Ctx>
where
    Ctx::Height: BorshDeserialize,
    Ctx::Address: BorshDeserialize,
    Ctx::Value: BorshDeserialize,
{
    fn deserialize_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let height = Ctx::Height::deserialize_reader(reader)?;
        let round = Round::deserialize_reader(reader)?;
        let valid_round = Round::deserialize_reader(reader)?;
        let proposer = Ctx::Address::deserialize_reader(reader)?;
        let value = Ctx::Value::deserialize_reader(reader)?;
        let validity = Validity::deserialize_reader(reader)?;
        Ok(ProposedValue {
            height,
            round,
            valid_round,
            proposer,
            value,
            validity,
        })
    }
}
