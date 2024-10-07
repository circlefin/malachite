use std::io;
use std::marker::PhantomData;

use async_trait::async_trait;
use derive_where::derive_where;
use futures_util::io::AsyncReadExt;
use futures_util::io::AsyncWriteExt;
use libp2p::futures::{AsyncRead, AsyncWrite};
use libp2p::request_response;
use libp2p::StreamProtocol;

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

#[derive_where(Clone)]
pub struct RpcCodec<Ctx, N> {
    marker: PhantomData<(Ctx, N)>,
}

impl<Ctx, N> Default for RpcCodec<Ctx, N> {
    fn default() -> Self {
        Self {
            marker: PhantomData,
        }
    }
}

/// Max request size in bytes
const REQUEST_SIZE_MAXIMUM: u64 = 1024 * 1024;
/// Max response size in bytes
const RESPONSE_SIZE_MAXIMUM: u64 = 10 * 1024 * 1024;

#[async_trait]
impl<Ctx, N> request_response::Codec for RpcCodec<Ctx, N>
where
    Ctx: Context,
    N: NetworkCodec<Ctx>,
{
    type Protocol = StreamProtocol;
    type Request = Request<Ctx>;
    type Response = Response<Ctx>;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut vec = Vec::new();
        io.take(REQUEST_SIZE_MAXIMUM).read_to_end(&mut vec).await?;

        N::decode_request(vec).map_err(encode_into_io_error)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut vec = Vec::new();
        io.take(RESPONSE_SIZE_MAXIMUM).read_to_end(&mut vec).await?;

        N::decode_response(vec).map_err(encode_into_io_error)
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = N::encode_request(req).map_err(encode_into_io_error)?;
        io.write_all(data.as_ref()).await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        resp: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        let data = N::encode_response(resp).map_err(encode_into_io_error)?;
        io.write_all(data.as_ref()).await?;

        Ok(())
    }
}

fn encode_into_io_error<E>(err: E) -> io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    io::Error::new(io::ErrorKind::Other, err)
}
