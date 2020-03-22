use bitflags::bitflags;

bitflags! {
    pub struct PlayerDirection: u8 {
        const LEFT  = 0b00000001;
        const RIGHT = 0b00000010;
        const UP    = 0b00000100;
        const DOWN  = 0b00001000;
    }
}

#[derive(Clone, Copy)]
pub struct PlayerInput {
    pub direction: PlayerDirection,
    pub button: bool,
    pub code: bool,
    pub pause: bool,
    pub quit: bool,
    pub last_char: char,
    pub save: bool,
    pub load: bool,
    pub state_slot: i8,
}

impl PlayerInput {
    pub fn new() -> PlayerInput {
        PlayerInput {
            direction: PlayerDirection::empty(),
            button: false,
            code: false,
            pause: false,
            quit: false,
            last_char: '\0',
            save: false,
            load: false,
            state_slot: 0,
        }
    }
}
