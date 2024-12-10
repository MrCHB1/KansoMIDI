use std::{fs::File, io::Read};
use std::io::{self, Seek};
use std::sync::{Arc, Mutex};

pub struct BufferedByteReader {
    pub file_stream: Arc<Mutex<File>>,
    start: usize,
    len: usize,
    buf_size: usize,
    pub pos: usize,
    buf_start: usize,
    buf_pos: usize,
    buf: Vec<u8>
}

impl BufferedByteReader {
    pub fn new(stream: Arc<Mutex<File>>, start: usize, len: usize, buf_size: usize) -> Result<Self, ()> {
        let mut buffer_length = buf_size;
        if buffer_length > len { buffer_length = len; }
        
        let mut bbr = Self {
            file_stream: stream,
            start,
            len,
            buf_size,
            pos: start,
            buf_start: 0,
            buf_pos: 0,
            buf: vec![0; buffer_length]
        };
        
        bbr.update_buffer().unwrap();

        Ok(bbr)
    }

    fn update_buffer(&mut self) -> Result<(),&str> {
        let mut read = self.buf_size as usize;

        if (self.pos + read) > (self.start + self.len) {
            read = self.start + self.len - self.pos;
        }

        if read == 0 && self.buf_size != 0 {
            panic!("outside buffer");
        }

        // lol
        {
            let mut strm = self.file_stream.lock().unwrap();
            strm.seek(io::SeekFrom::Start(self.pos as u64)).unwrap();
            strm.read(&mut self.buf).unwrap();
        }

        self.buf_start = self.pos;
        self.buf_pos = 0usize;

        Ok(())
    }

    pub fn seek(&mut self, offset: isize, origin: i32) -> Result<(), &str> {
        let mut real_offs: isize = offset;
        if origin == 0 {
            real_offs += self.start as isize;
        } else {
            real_offs += self.pos as isize;
        }

        if real_offs < self.start as isize {
            panic!("seek before start")
        }
        if real_offs > (self.start + self.len) as isize {
            panic!("seek past end")
        }

        self.pos = real_offs as usize;

        if self.buf_start as isize <= real_offs && (real_offs) < (self.buf_start + self.buf_size) as isize {
            self.buf_pos = self.pos - self.buf_start;
            return Ok(())
        }

        self.update_buffer().unwrap();

        Ok(())
    }

    pub fn read(&mut self, dst: &mut [u8], size: usize) -> Result<(), &str> {
        if self.pos + size > self.start + self.len {
            panic!("read past end | requested read: {}, buff size: {}", self.pos + size, self.start + self.len);
            //return Err(format!("read past end | requested read: {}, buff size: {}", self.pos + size, self.start + self.len))
        }
        if size > self.buf_size as usize {
            //panic!("unimplemented; read size larger than buffer size");
            return Err("unimplemented; read size larger than buffer size")
        }

        if self.buf_start + self.buf_pos + size > self.buf_start + self.buf_size as usize {
            self.update_buffer().unwrap();
        }

        // skull emoji
        dst[..size].clone_from_slice(&self.buf[self.buf_pos..self.buf_pos+size]);
        self.pos += size;
        self.buf_pos += size;

        Ok(())
    }

    pub fn read_byte(&mut self) -> Result<u8,()> {
        let mut ret: [u8; 1] = [0];
        self.read(&mut ret, 1).unwrap();
        Ok(ret[0])
    }

    pub fn skip_bytes(&mut self, size: usize) -> Result<(),()> {
        self.seek(size as isize, 1).unwrap();
        Ok(())
    }
}