use bytes::Bytes;
use malachite_common::Context;

use crate::{Request, Response, Status};

pub trait NetworkCodec<Ctx: Context>: Sync + Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn decode_status(bytes: Bytes) -> Result<Status<Ctx>, Self::Error>;
    fn encode_status(status: Status<Ctx>) -> Result<Bytes, Self::Error>;

    fn decode_request(bytes: Bytes) -> Result<Request<Ctx>, Self::Error>;
    fn encode_request(request: Request<Ctx>) -> Result<Bytes, Self::Error>;

    fn decode_response(bytes: Bytes) -> Result<Response<Ctx>, Self::Error>;
    fn encode_response(response: Response<Ctx>) -> Result<Bytes, Self::Error>;
}
