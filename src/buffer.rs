pub struct Buffer<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Buffer<'a> {
    pub fn new(data: &[u8]) -> Buffer {
        Buffer {
            data,
            offset: 0,
        }
    }

    pub fn with_offset(data: &[u8], offset: usize) -> Buffer {
        Buffer {
            data,
            offset,
        }
    }

    pub fn fetch_byte(&mut self) -> u8 {
        let result = self.data[self.offset];
        self.offset += 1;
        result
    }
}

