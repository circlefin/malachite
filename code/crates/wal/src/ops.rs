use std::io::{Read, Seek, Write};

pub trait FileOps: Read + Write + Seek {
    fn metadata(&self) -> std::io::Result<std::fs::Metadata>;

    fn set_len(&mut self, size: u64) -> std::io::Result<()>;

    fn sync_all(&mut self) -> std::io::Result<()>;
}
