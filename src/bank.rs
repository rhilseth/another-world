use std::mem;

use byteorder::{ByteOrder, BigEndian};
use log::debug;

pub enum Bank {
    Uncompressed(Vec<u8>),
    Compressed(Vec<u8>),
}

impl Bank {
    pub fn data(self) -> Vec<u8> {
        match self {
            Bank::Uncompressed(data) => data,
            Bank::Compressed(data) => {
                let mut unpacker = Unpacker::new(&data);
                unpacker.unpack()
            }
        }
    }
}

struct Unpacker<'a> {
    data: &'a [u8],
    i: usize,
    size: u32,
    datasize: u32,
    crc: u32,
    chk: u32,
    output: Vec<u8>,
}

impl<'a> Unpacker<'a> {
    fn new(data: &'a [u8]) -> Unpacker {
        Unpacker {
            data,
            i: 0,
            size: 0,
            datasize: 0,
            crc: 0,
            chk: 0,
            output: Vec::new(),
        }
    }

    fn read_reverse_be_u32(&mut self) -> u32 {
        let result = BigEndian::read_u32(&self.data[self.i..]);
        if self.i >= 4 {
            self.i -= 4;
        }
        result
    }

    fn next_chunk(&mut self) -> bool {
        let mut cf = self.rcr(false);
        if self.chk == 0 {
            debug!("i = {}", self.i);
            self.chk = self.read_reverse_be_u32();
            self.crc ^= self.chk;
            cf = self.rcr(true);
        }
        cf
    }

    fn dec_unk1(&mut self, num_chunks: u32, add_count: u32) {
        let mut count = self.get_code(num_chunks) + add_count + 1;
        debug!("dec_unk1({}, {}) count={}", num_chunks, add_count, count);
        self.datasize -= count;
        while count > 0 {
            count -= 1;
            let val = self.get_code(8) as u8;
            self.output.push(val);
        }
    }

    fn dec_unk2(&mut self, num_chunks: u32) {
        let i = self.get_code(num_chunks) as usize;
        let mut count = self.size + 1;
        debug!("dec_unk2({}) i={} count={}", num_chunks, i, count);
        self.datasize -= count;
        while count > 0 {
            count -= 1;
            let val = self.output[self.output.len() - i];
            self.output.push(val);
        }
    }

    fn get_code(&mut self, num_chunks: u32) -> u32 {
        let mut num_chunks = num_chunks;
        let mut c = 0;
        while num_chunks > 0 {
            num_chunks -= 1;
            c <<= 1;
            if self.next_chunk() {
                c |= 1;
            }
        }
        c
    }

    fn rcr(&mut self, cf: bool) -> bool {
        let rcf: bool = (self.chk & 1) > 0;
        self.chk >>= 1;
        if cf {
            self.chk |= 0x80000000;
        }
        rcf
    }

    fn unpack(&mut self) -> Vec<u8> {
        debug!("Unpack()");
        self.i = self.data.len() - 4;
        self.size = 0;
        self.datasize = self.read_reverse_be_u32();
        self.crc = self.read_reverse_be_u32();
        self.chk = self.read_reverse_be_u32();
        self.crc ^= self.chk;
        while self.datasize > 0 {
            if !self.next_chunk() {
                self.size = 1;
                if !self.next_chunk() {
                    self.dec_unk1(3, 0);
                } else {
                    self.dec_unk2(8);
                }
            } else {
                let c = self.get_code(2);
                if c == 3 {
                    self.dec_unk1(8, 8);
                } else {
                    if c < 2 {
                        self.size = c + 2;
                        self.dec_unk2(c + 9);
                    } else {
                        self.size = self.get_code(8);
                        self.dec_unk2(12);
                    }
                }
            }
        }
        if self.crc != 0 {
            panic!("CRC Error: {}", self.crc);
        }
        self.output.reverse();
        let mut new_output = Vec::new();
        mem::swap(&mut self.output, &mut new_output);
        new_output
    }
}

