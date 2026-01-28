use crate::util::streaming::StreamId;

impl borsh::BorshSerialize for StreamId {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.0.to_vec().serialize(writer)
    }
}

impl borsh::BorshDeserialize for StreamId {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let bytes = Vec::<u8>::deserialize_reader(reader)?;
        Ok(StreamId(bytes.into()))
    }
}
