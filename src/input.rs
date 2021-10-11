use sdl2::EventPump;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;


use crate::player::{PlayerDirection, PlayerInput};

pub struct UserInput {
    event_pump: EventPump,
    player_input: PlayerInput,
}

impl UserInput {
    pub fn new(event_pump: EventPump) -> Self {
        Self {
            event_pump,
            player_input: PlayerInput::new(),
        }
    }

    pub fn process_events(&mut self) -> PlayerInput {
        let mut last_char = '\0';
        for event in self.event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => self.player_input.quit = true,
                Event::KeyDown { keycode, .. } => match keycode.unwrap() {
                    Keycode::Left => self.player_input.direction |= PlayerDirection::LEFT,
                    Keycode::Right => self.player_input.direction |= PlayerDirection::RIGHT,
                    Keycode::Up => self.player_input.direction |= PlayerDirection::UP,
                    Keycode::Down => self.player_input.direction |= PlayerDirection::DOWN,
                    Keycode::LShift | Keycode::Space | Keycode::Return => {
                        self.player_input.button = true
                    }
                    Keycode::Backspace => last_char = '\x08',
                    Keycode::A => {
                        self.player_input.direction |= PlayerDirection::LEFT;
                        last_char = 'A';
                    }
                    Keycode::B => last_char = 'B',
                    Keycode::C => {
                        self.player_input.code = true;
                        last_char = 'C';
                    }
                    Keycode::D => {
                        self.player_input.direction |= PlayerDirection::RIGHT;
                        last_char = 'D';
                    }
                    Keycode::E => last_char = 'E',
                    Keycode::F => last_char = 'F',
                    Keycode::G => last_char = 'G',
                    Keycode::H => last_char = 'H',
                    Keycode::I => last_char = 'I',
                    Keycode::J => last_char = 'J',
                    Keycode::K => last_char = 'K',
                    Keycode::L => last_char = 'L',
                    Keycode::M => last_char = 'M',
                    Keycode::N => last_char = 'N',
                    Keycode::O => last_char = 'O',
                    Keycode::P => last_char = 'P',
                    Keycode::Q => last_char = 'Q',
                    Keycode::R => last_char = 'R',
                    Keycode::S => {
                        self.player_input.direction |= PlayerDirection::DOWN;
                        last_char = 'S';
                    }
                    Keycode::T => last_char = 'T',
                    Keycode::U => last_char = 'U',
                    Keycode::V => last_char = 'V',
                    Keycode::W => {
                        self.player_input.direction |= PlayerDirection::UP;
                        last_char = 'W';
                    }
                    Keycode::X => last_char = 'X',
                    Keycode::Y => last_char = 'Y',
                    Keycode::Z => last_char = 'Z',
                    _ => {}
                },
                Event::KeyUp { keycode, .. } => match keycode.unwrap() {
                    Keycode::Left | Keycode::A => {
                        self.player_input.direction &= !PlayerDirection::LEFT
                    }
                    Keycode::Right | Keycode::D => {
                        self.player_input.direction &= !PlayerDirection::RIGHT
                    }
                    Keycode::Up | Keycode::W => self.player_input.direction &= !PlayerDirection::UP,
                    Keycode::Down | Keycode::S => {
                        self.player_input.direction &= !PlayerDirection::DOWN
                    }
                    Keycode::LShift | Keycode::Space | Keycode::Return => {
                        self.player_input.button = false
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        self.player_input.last_char = last_char;
        let result = self.player_input;
        self.player_input.code = false;
        result
    }
}


