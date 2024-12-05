use std::io::{self, Read, Write};

#[inline(always)]
pub fn read_u8<R: Read>(reader: &mut R) -> io::Result<u8> {
    let mut buf = [0; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0])
}

#[inline(always)]
pub fn write_u8<W: Write>(writer: &mut W, value: u8) -> io::Result<()> {
    writer.write_all(&[value])
}

#[inline(always)]
pub fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut buf = [0; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

#[inline(always)]
pub fn write_u32<W: Write>(writer: &mut W, value: u32) -> io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

#[inline(always)]
pub fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut buf = [0; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_be_bytes(buf))
}

#[inline(always)]
pub fn write_u64<W: Write>(writer: &mut W, value: u64) -> io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}
