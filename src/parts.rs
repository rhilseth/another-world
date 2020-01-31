pub struct Part {
    pub palette: usize,
    pub code: usize,
    pub video1: usize,
    pub video2: Option<usize>,
}

pub const PARTS: [Part; 10] = [
    Part { palette: 0x14, code: 0x15, video1: 0x16, video2: None }, // protection screens
    Part { palette: 0x17, code: 0x18, video1: 0x19, video2: None }, // introduction cinematic
    Part { palette: 0x1A, code: 0x1B, video1: 0x1C, video2: Some(0x11) },
    Part { palette: 0x1D, code: 0x1E, video1: 0x1F, video2: Some(0x11) },
    Part { palette: 0x20, code: 0x21, video1: 0x22, video2: Some(0x11) },
    Part { palette: 0x23, code: 0x24, video1: 0x25, video2: None }, // battlechar cinematic
    Part { palette: 0x26, code: 0x27, video1: 0x28, video2: Some(0x11) },
    Part { palette: 0x29, code: 0x2A, video1: 0x2B, video2: Some(0x11) },
    Part { palette: 0x7D, code: 0x7E, video1: 0x7F, video2: None },
    Part { palette: 0x7D, code: 0x7E, video1: 0x7F, video2: None }  // password screen
];

pub const GAME_PART_FIRST: u16 = 0x3e80;
pub const GAME_PART_LAST: u16 = 0x3e89;
