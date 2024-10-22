use bytes::Bytes;
use malachite_proto as proto;
use malachite_starknet_p2p_proto as p2p_proto;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockProof {
    pub bytes: Vec<Bytes>,
}

impl BlockProof {
    pub fn new(bytes: Vec<Bytes>) -> Self {
        Self { bytes }
    }
}

impl proto::Protobuf for BlockProof {
    type Proto = p2p_proto::BlockProof;

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn from_proto(proto: Self::Proto) -> Result<Self, proto::Error> {
        Ok(Self { bytes: proto.proof })
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn to_proto(&self) -> Result<Self::Proto, proto::Error> {
        Ok(Self::Proto {
            proof: self.bytes.clone(),
        })
    }
}
