use std::{thread, time};

use sdl2::timer;

use crate::video::Page;

pub struct SDLSys {

}

impl SDLSys {
    pub fn new() -> SDLSys {
        SDLSys { }
    }

    pub fn update_display(&self, page: &Page) {
    }

    pub fn sleep(&self, ms: u64) {
        let duration = time::Duration::from_millis(ms);
        thread::sleep(duration);
    }

    pub fn get_timestamp(&self) -> u64 {
        0
    }
}
