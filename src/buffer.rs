use byteorder::{BigEndian, ByteOrder};

pub struct Buffer<'a> {
    data: &'a [u8],
    pub offset: usize,
}

impl<'a> Buffer<'a> {
    pub fn with_offset(data: &[u8], offset: usize) -> Buffer {
        Buffer { data, offset }
    }

    pub fn fetch_byte(&mut self) -> u8 {
        let result = self.data[self.offset];
        self.offset += 1;
        result
    }

    pub fn fetch_word(&mut self) -> u16 {
        let result = BigEndian::read_u16(&self.data[self.offset..]);
        self.offset += 2;
        result
    }
}
