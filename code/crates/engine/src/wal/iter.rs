use std::io;
use std::marker::PhantomData;

use malachitebft_core_types::Context;
use malachitebft_wal as wal;

use eyre::Result;

use super::entry::decode_entry;
use super::{WalCodec, WalEntry};

pub fn log_entries<'a, Ctx, Codec>(
    log: &'a mut wal::Log,
    codec: &'a Codec,
) -> Result<WalIter<'a, Ctx, Codec>>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    Ok(WalIter {
        iter: log.iter()?,
        codec,
        _marker: PhantomData,
    })
}

pub struct WalIter<'a, Ctx, Codec> {
    iter: wal::LogIter<'a>,
    codec: &'a Codec,
    _marker: PhantomData<Ctx>,
}

impl<Ctx, Codec> Iterator for WalIter<'_, Ctx, Codec>
where
    Ctx: Context,
    Codec: WalCodec<Ctx>,
{
    type Item = Result<WalEntry<Ctx>>;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.iter.next()?;
        match entry {
            Ok(bytes) => {
                let buf = io::Cursor::new(bytes);
                let entry = decode_entry(self.codec, buf);
                Some(entry.map_err(Into::into))
            }
            Err(e) => Some(Err(e.into())),
        }
    }
}
