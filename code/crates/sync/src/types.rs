use std::sync::Arc;

use bytes::Bytes;
use derive_where::derive_where;
use displaydoc::Display;
use libp2p::request_response;
use serde::{Deserialize, Serialize};

use malachitebft_core_types::{CommitCertificate, Context};
pub use malachitebft_peer::PeerId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct InboundRequestId(Arc<str>);

impl InboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(Arc::from(id.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Display)]
#[displaydoc("{0}")]
pub struct OutboundRequestId(Arc<str>);

impl OutboundRequestId {
    pub fn new(id: impl ToString) -> Self {
        Self(Arc::from(id.to_string()))
    }
}

pub type ResponseChannel = request_response::ResponseChannel<RawResponse>;

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct Status<Ctx: Context> {
    pub peer_id: PeerId,
    pub tip_height: Ctx::Height,
    pub history_min_height: Ctx::Height,
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Request<Ctx: Context> {
    ValueRequest(ValueRequest<Ctx>),
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub enum Response<Ctx: Context> {
    ValueResponse(ValueResponse<Ctx>),
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueRequest<Ctx: Context> {
    pub height: Ctx::Height,
}

impl<Ctx: Context> ValueRequest<Ctx> {
    pub fn new(height: Ctx::Height) -> Self {
        Self { height }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct ValueResponse<Ctx: Context> {
    pub height: Ctx::Height,
    pub value: Option<RawDecidedValue<Ctx>>,
}

impl<Ctx: Context> ValueResponse<Ctx> {
    pub fn new(height: Ctx::Height, value: Option<RawDecidedValue<Ctx>>) -> Self {
        Self { height, value }
    }
}

#[derive_where(Clone, Debug, PartialEq, Eq)]
pub struct RawDecidedValue<Ctx: Context> {
    pub value_bytes: Bytes,
    pub certificate: CommitCertificate<Ctx>,
}

impl<Ctx: Context> RawDecidedValue<Ctx> {
    pub fn new(value_bytes: Bytes, certificate: CommitCertificate<Ctx>) -> Self {
        Self {
            value_bytes,
            certificate,
        }
    }
}

#[derive(Clone, Debug)]
pub enum RawMessage {
    Request {
        request_id: request_response::InboundRequestId,
        peer: PeerId,
        body: Bytes,
    },
    Response {
        request_id: request_response::OutboundRequestId,
        peer: PeerId,
        body: Bytes,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawRequest(pub Bytes);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawResponse(pub Bytes);

#[cfg(feature = "borsh")]
mod _borsh {
    use borsh::BorshSerialize;

    use super::*;

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
                Request::ValueRequest(value_request) => value_request.height.serialize(writer),
            }
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for Request<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height = Ctx::Height::deserialize_reader(reader)?;
            Ok(Request::ValueRequest(ValueRequest::new(height)))
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
            self.height.serialize(writer)?;
            self.value.serialize(writer)?;
            Ok(())
        }
    }

    impl<Ctx: Context> borsh::BorshDeserialize for ValueResponse<Ctx>
    where
        Ctx::Height: borsh::BorshDeserialize,
        RawDecidedValue<Ctx>: borsh::BorshDeserialize,
    {
        fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
            let height: <Ctx as Context>::Height = Ctx::Height::deserialize_reader(reader)?;
            let value = Option::<RawDecidedValue<Ctx>>::deserialize_reader(reader)?;
            Ok(ValueResponse { height, value })
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
}
