use async_trait::async_trait;
use bytes::Bytes;
use libp2p::futures::{io, AsyncRead, AsyncWrite};
use libp2p::StreamProtocol;

use crate::types::{RawRequest, RawResponse};
use crate::Config;

#[derive(Copy, Clone)]
pub struct Codec {
    config: Config,
}

impl Codec {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl libp2p::request_response::Codec for Codec {
    type Protocol = StreamProtocol;

    type Request = RawRequest;
    type Response = RawResponse;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed(io, self.config.max_request_size)
            .await
            .map(RawRequest)
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_length_prefixed(io, self.config.max_response_size)
            .await
            .map(RawResponse)
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
        write_length_prefixed(io, req.0, self.config.max_request_size).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, res.0, self.config.max_response_size).await
    }
}

const U32_LENGTH: usize = size_of::<u32>();

async fn write_length_prefixed<T>(dst: &mut T, data: Bytes, max_len: usize) -> io::Result<()>
where
    T: AsyncWrite + Unpin + Send,
{
    use io::AsyncWriteExt;

    let len = data.len();
    if len > max_len || len > u32::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "data too large",
        ));
    }

    dst.write_all(&(len as u32).to_be_bytes()).await?;
    dst.write_all(&data).await?;
    dst.flush().await?;

    Ok(())
}

async fn read_length_prefixed<T>(src: &mut T, max_len: usize) -> io::Result<Bytes>
where
    T: AsyncRead + Unpin + Send,
{
    use io::AsyncReadExt;

    let mut len_bytes = [0u8; U32_LENGTH];
    src.read_exact(&mut len_bytes).await?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    if len > max_len {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "data too large"));
    }

    let mut data = vec![0u8; len];
    src.read_exact(&mut data).await?;
    Ok(Bytes::from(data))
}
