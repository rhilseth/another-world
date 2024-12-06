use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy)]
    pub struct PlayerDirection: u8 {
        const LEFT  = 0b0000_0001;
        const RIGHT = 0b0000_0010;
        const UP    = 0b0000_0100;
        const DOWN  = 0b0000_1000;
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

impl Default for PlayerInput {
    fn default() -> Self {
        Self::new()
    }
}
