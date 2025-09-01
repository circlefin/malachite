use malachitebft_core_types::CommitCertificate;
use malachitebft_proto::Error as ProtoError;
use prost::Message;

use crate::codec;
use crate::types::{proto, Block, MockContext};

#[derive(Clone, Debug)]
pub struct DecidedBlock {
    pub block: Block,
    pub certificate: CommitCertificate<MockContext>,
}

pub fn decode_certificate(bytes: &[u8]) -> Result<CommitCertificate<MockContext>, ProtoError> {
    let proto = proto::CommitCertificate::decode(bytes)?;
    codec::decode_commit_certificate(proto)
}

pub fn encode_certificate(
    certificate: &CommitCertificate<MockContext>,
) -> Result<Vec<u8>, ProtoError> {
    let proto = codec::encode_commit_certificate(certificate)?;
    Ok(proto.encode_to_vec())
}
