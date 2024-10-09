use malachite_common::Context;

use crate::{Request, Response, Status};

pub trait NetworkCodec<Ctx: Context>: Sync + Send + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn decode_status(bytes: Vec<u8>) -> Result<Status<Ctx>, Self::Error>;
    fn encode_status(status: Status<Ctx>) -> Result<Vec<u8>, Self::Error>;

    fn decode_request(bytes: Vec<u8>) -> Result<Request<Ctx>, Self::Error>;
    fn encode_request(request: Request<Ctx>) -> Result<Vec<u8>, Self::Error>;

    fn decode_response(bytes: Vec<u8>) -> Result<Response<Ctx>, Self::Error>;
    fn encode_response(response: Response<Ctx>) -> Result<Vec<u8>, Self::Error>;
}
