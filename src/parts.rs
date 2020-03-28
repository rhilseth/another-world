pub struct Part {
    pub palette: usize,
    pub code: usize,
    pub video1: usize,
    pub video2: Option<usize>,
}

pub const PARTS: [Part; 10] = [
    Part {
        palette: 0x14,
        code: 0x15,
        video1: 0x16,
        video2: None,
    }, // protection screens
    Part {
        palette: 0x17,
        code: 0x18,
        video1: 0x19,
        video2: None,
    }, // introduction cinematic
    Part {
        palette: 0x1A,
        code: 0x1B,
        video1: 0x1C,
        video2: Some(0x11),
    },
    Part {
        palette: 0x1D,
        code: 0x1E,
        video1: 0x1F,
        video2: Some(0x11),
    },
    Part {
        palette: 0x20,
        code: 0x21,
        video1: 0x22,
        video2: Some(0x11),
    },
    Part {
        palette: 0x23,
        code: 0x24,
        video1: 0x25,
        video2: None,
    }, // battlechar cinematic
    Part {
        palette: 0x26,
        code: 0x27,
        video1: 0x28,
        video2: Some(0x11),
    },
    Part {
        palette: 0x29,
        code: 0x2A,
        video1: 0x2B,
        video2: Some(0x11),
    },
    Part {
        palette: 0x7D,
        code: 0x7E,
        video1: 0x7F,
        video2: None,
    },
    Part {
        palette: 0x7D,
        code: 0x7E,
        video1: 0x7F,
        video2: None,
    }, // password screen
];

pub const GAME_PART1: u16 = 0x3E80;
pub const GAME_PART2: u16 = 0x3E81; //Introduction
pub const GAME_PART3: u16 = 0x3E82;
pub const GAME_PART4: u16 = 0x3E83; //Wake up in the suspended jail
pub const GAME_PART5: u16 = 0x3E84;
pub const GAME_PART6: u16 = 0x3E85; //BattleChar sequence
pub const GAME_PART7: u16 = 0x3E86;
pub const GAME_PART8: u16 = 0x3E87;
pub const GAME_PART9: u16 = 0x3E88;
pub const GAME_PART10: u16 = 0x3E89;

pub const GAME_PART_FIRST: u16 = GAME_PART1;
pub const GAME_PART_LAST: u16 = GAME_PART10;
