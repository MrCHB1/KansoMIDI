use std::{fs::File, io::Read};
use std::io;

pub fn read_u32(stream: &mut File) -> io::Result<u32> {
    let mut buf: [u8; 4] = [0; 4];
    stream.read_exact(&mut buf[..])?;
    Ok(u32::from_be_bytes(buf))
}

pub fn read_u16(stream: &mut File) -> io::Result<u16> {
    let mut buf: [u8; 2] = [0, 0];
    stream.read_exact(&mut buf[..])?;
    Ok(u16::from_be_bytes(buf))
}