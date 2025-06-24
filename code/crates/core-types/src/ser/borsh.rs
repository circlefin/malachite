use {
    crate::{
        CommitCertificate, CommitSignature, Context, NilOrVal, PolkaCertificate, PolkaSignature,
        Round, RoundCertificate, RoundCertificateType, RoundSignature, Signature, SignedMessage,
        ValueId, VoteType,
    },
    ::borsh::BorshSerialize,
    alloc::vec::Vec,
};

impl<Ctx: Context> BorshSerialize for PolkaSignature<Ctx>
where
    Ctx::Address: BorshSerialize,
    Signature<Ctx>: BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.address.serialize(writer)?;
        self.signature.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for PolkaSignature<Ctx>
where
    Ctx::Address: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let address = Ctx::Address::deserialize_reader(reader)?;
        let signature = Signature::<Ctx>::deserialize_reader(reader)?;
        Ok(PolkaSignature { address, signature })
    }
}

impl<Ctx: Context> ::borsh::BorshSerialize for PolkaCertificate<Ctx>
where
    Ctx::Address: borsh::BorshSerialize,
    Ctx::Height: borsh::BorshSerialize,
    Signature<Ctx>: borsh::BorshSerialize,
    ValueId<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.height.serialize(writer)?;
        self.round.serialize(writer)?;
        self.value_id.serialize(writer)?;
        self.polka_signatures.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for PolkaCertificate<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
    Ctx::Address: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
    ValueId<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let height = Ctx::Height::deserialize_reader(reader)?;
        let round = Round::deserialize_reader(reader)?;
        let value_id = ValueId::<Ctx>::deserialize_reader(reader)?;
        let polka_signatures = Vec::<PolkaSignature<Ctx>>::deserialize_reader(reader)?;
        Ok(PolkaCertificate {
            height,
            round,
            value_id,
            polka_signatures,
        })
    }
}

impl<Ctx: Context> ::borsh::BorshSerialize for RoundCertificate<Ctx>
where
    Ctx::Height: borsh::BorshSerialize,
    Ctx::Address: borsh::BorshSerialize,
    Signature<Ctx>: borsh::BorshSerialize,
    RoundSignature<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.height.serialize(writer)?;
        self.round.serialize(writer)?;
        self.cert_type.serialize(writer)?;
        self.round_signatures.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for RoundCertificate<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
    Ctx::Address: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
    RoundSignature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let height = Ctx::Height::deserialize_reader(reader)?;
        let round = Round::deserialize_reader(reader)?;
        let cert_type = RoundCertificateType::deserialize_reader(reader)?;
        let round_signatures = Vec::<RoundSignature<Ctx>>::deserialize_reader(reader)?;
        Ok(RoundCertificate {
            height,
            round,
            cert_type,
            round_signatures,
        })
    }
}

impl<Ctx: Context> ::borsh::BorshSerialize for RoundSignature<Ctx>
where
    NilOrVal<ValueId<Ctx>>: borsh::BorshSerialize,
    Ctx::Address: borsh::BorshSerialize,
    Signature<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.vote_type.serialize(writer)?;
        self.value_id.serialize(writer)?;
        self.address.serialize(writer)?;
        self.signature.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for RoundSignature<Ctx>
where
    NilOrVal<ValueId<Ctx>>: borsh::BorshDeserialize,
    Ctx::Address: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let vote_type = VoteType::deserialize_reader(reader)?;
        let value_id = NilOrVal::<ValueId<Ctx>>::deserialize_reader(reader)?;
        let address = Ctx::Address::deserialize_reader(reader)?;
        let signature = Signature::<Ctx>::deserialize_reader(reader)?;
        Ok(RoundSignature {
            vote_type,
            value_id,
            address,
            signature,
        })
    }
}

impl<Ctx: Context> ::borsh::BorshSerialize for CommitCertificate<Ctx>
where
    Ctx::Height: borsh::BorshSerialize,
    ValueId<Ctx>: borsh::BorshSerialize,
    CommitSignature<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.height.serialize(writer)?;
        self.round.serialize(writer)?;
        self.value_id.serialize(writer)?;
        self.commit_signatures.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for CommitCertificate<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
    ValueId<Ctx>: borsh::BorshDeserialize,
    CommitSignature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let height = Ctx::Height::deserialize_reader(reader)?;
        let round = Round::deserialize_reader(reader)?;
        let value_id = ValueId::<Ctx>::deserialize_reader(reader)?;
        let commit_signatures = Vec::<CommitSignature<Ctx>>::deserialize_reader(reader)?;
        Ok(CommitCertificate {
            height,
            round,
            value_id,
            commit_signatures,
        })
    }
}

impl<Ctx: Context> ::borsh::BorshSerialize for CommitSignature<Ctx>
where
    Ctx::Address: borsh::BorshSerialize,
    Signature<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.address.serialize(writer)?;
        self.signature.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> ::borsh::BorshDeserialize for CommitSignature<Ctx>
where
    Ctx::Address: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let address = Ctx::Address::deserialize_reader(reader)?;
        let signature = Signature::<Ctx>::deserialize_reader(reader)?;
        Ok(CommitSignature { address, signature })
    }
}

impl<Ctx, Msg> borsh::BorshSerialize for SignedMessage<Ctx, Msg>
where
    Ctx: Context,
    Msg: borsh::BorshSerialize,
    Signature<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.message.serialize(writer)?;
        self.signature.serialize(writer)?;
        Ok(())
    }
}

#[cfg(feature = "borsh")]
impl<Ctx, Msg> borsh::BorshDeserialize for SignedMessage<Ctx, Msg>
where
    Ctx: Context,
    Msg: borsh::BorshDeserialize,
    Signature<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        Ok(Self {
            message: Msg::deserialize_reader(reader)?,
            signature: Signature::<Ctx>::deserialize_reader(reader)?,
        })
    }
}
