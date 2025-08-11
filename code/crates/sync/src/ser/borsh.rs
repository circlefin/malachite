use {
    crate::{RawDecidedValue, Request, Response, Status, ValueRequest, ValueResponse},
    borsh::BorshSerialize,
    malachitebft_core_types::{CommitCertificate, Context},
    malachitebft_peer::PeerId,
    std::ops::RangeInclusive,
};

impl<Ctx: Context> borsh::BorshSerialize for Status<Ctx>
where
    Ctx::Height: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.peer_id.serialize(writer)?;
        self.tip_height.serialize(writer)?;
        self.history_min_height.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> borsh::BorshDeserialize for Status<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let peer_id = PeerId::deserialize_reader(reader)?;
        let tip_height = Ctx::Height::deserialize_reader(reader)?;
        let history_min_height = Ctx::Height::deserialize_reader(reader)?;
        Ok(Status {
            peer_id,
            tip_height,
            history_min_height,
        })
    }
}

impl<Ctx: Context> borsh::BorshSerialize for Request<Ctx>
where
    Ctx::Height: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        match self {
            Request::ValueRequest(value_request) => value_request.range.serialize(writer),
        }
    }
}

impl<Ctx: Context> borsh::BorshDeserialize for Request<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let range = RangeInclusive::<Ctx::Height>::deserialize_reader(reader)?;
        Ok(Request::ValueRequest(ValueRequest::new(range)))
    }
}

impl<Ctx: Context> borsh::BorshSerialize for Response<Ctx>
where
    ValueResponse<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        match self {
            Response::ValueResponse(value_response) => value_response.serialize(writer),
        }
    }
}

impl<Ctx: Context> borsh::BorshDeserialize for Response<Ctx>
where
    ValueResponse<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let value = ValueResponse::deserialize_reader(reader)?;
        Ok(Response::ValueResponse(value))
    }
}

impl<Ctx: Context> borsh::BorshSerialize for ValueResponse<Ctx>
where
    Ctx::Height: borsh::BorshSerialize,
    RawDecidedValue<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.start_height.serialize(writer)?;
        self.values.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> borsh::BorshDeserialize for ValueResponse<Ctx>
where
    Ctx::Height: borsh::BorshDeserialize,
    RawDecidedValue<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let start_height = Ctx::Height::deserialize_reader(reader)?;
        let values = Vec::<RawDecidedValue<Ctx>>::deserialize_reader(reader)?;
        Ok(ValueResponse {
            start_height,
            values,
        })
    }
}

impl<Ctx: Context> borsh::BorshSerialize for RawDecidedValue<Ctx>
where
    CommitCertificate<Ctx>: borsh::BorshSerialize,
{
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        BorshSerialize::serialize(&self.value_bytes.to_vec(), writer)?;
        self.certificate.serialize(writer)?;
        Ok(())
    }
}

impl<Ctx: Context> borsh::BorshDeserialize for RawDecidedValue<Ctx>
where
    CommitCertificate<Ctx>: borsh::BorshDeserialize,
{
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let value_bytes = Vec::<u8>::deserialize_reader(reader)?;
        let certificate = CommitCertificate::deserialize_reader(reader)?;
        Ok(RawDecidedValue {
            value_bytes: value_bytes.into(),
            certificate,
        })
    }
}
