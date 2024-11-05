use bytes::Bytes;
use malachite_proto::Error;
use malachite_starknet_p2p_proto::Felt252;

pub type Felt = starknet_crypto::Felt;

pub trait FeltExt: Sized {
    fn from_proto(proto: Felt252) -> Result<Self, Error>;
    fn to_proto(&self) -> Result<Felt252, Error>;
}

impl FeltExt for Felt {
    fn from_proto(proto: Felt252) -> Result<Self, Error> {
        let mut felt = [0; 32];
        felt.copy_from_slice(&proto.elements);
        Ok(Self::from_bytes_be(&felt))
    }

    fn to_proto(&self) -> Result<Felt252, Error> {
        Ok(Felt252 {
            elements: Bytes::copy_from_slice(&self.to_bytes_be()),
        })
    }
}
