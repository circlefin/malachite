use {crate::PeerId, alloc::vec::Vec};

impl borsh::BorshSerialize for PeerId {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        self.multihash.to_bytes().serialize(writer)
    }
}

impl borsh::BorshDeserialize for PeerId {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let bytes = Vec::<u8>::deserialize_reader(reader)?;
        Ok(PeerId::from_bytes(&bytes).unwrap())
    }
}
