use log::debug;
use std::sync::{Arc, RwLock};
use std::{thread, time};

use sdl2::audio::{AudioDevice, AudioSpecDesired};
use sdl2::pixels::{Color, Palette, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use sdl2::surface::Surface;

use crate::mixer;
use crate::video;

const SCREEN_W: u32 = 320;
const SCREEN_H: u32 = 200;
const _SOUND_SAMPLE_RATE: u16 = 22050;

pub struct SDLSys {
    sdl_context: sdl2::Sdl,
    surface: Surface<'static>,
    canvas: WindowCanvas,
    audio_device: Option<AudioDevice<mixer::MixerAudio>>,
}

impl SDLSys {
    pub fn new(sdl_context: sdl2::Sdl) -> SDLSys {
        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem
            .window("Another world", 1024, 770)
            .position_centered()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().build().expect("Expected canvas");
        canvas
            .set_logical_size(SCREEN_W, SCREEN_H)
            .expect("Expected logical size");
        SDLSys {
            sdl_context,
            surface: Surface::new(SCREEN_W, SCREEN_H, PixelFormatEnum::Index8).unwrap(),
            canvas,
            audio_device: None,
        }
    }

    pub fn set_palette(&mut self, palette: &video::Palette) {
        debug!("set_palette()");
        let colors: Vec<Color> = palette
            .entries
            .iter()
            .map(|c| Color::RGBA(c.r, c.g, c.b, c.a))
            .collect();
        let sdl_palette = Palette::with_colors(&colors).unwrap();

        self.surface.set_palette(&sdl_palette).unwrap();
    }

    pub fn update_display(&mut self, page: &video::Page) {
        debug!("update_display()");
        let pitch = self.surface.pitch() as usize;
        self.surface.with_lock_mut(|p| {
            for j in 0..(SCREEN_H as usize) {
                let p_offset = pitch * j;
                let page_offset = j * ((SCREEN_W / 2) as usize);
                for i in 0..((SCREEN_W / 2) as usize) {
                    p[p_offset + (i * 2 + 0)] = page.data[page_offset + i] >> 4;
                    p[p_offset + (i * 2 + 1)] = page.data[page_offset + i] & 0xf;
                }
            }
        });
        let texture_creator = self.canvas.texture_creator();
        let texture = texture_creator
            .create_texture_from_surface(&*self.surface)
            .unwrap();
        self.canvas.clear();
        self.canvas
            .copy(&texture, None, Some(Rect::new(0, 0, SCREEN_W, SCREEN_H)))
            .unwrap();
        self.canvas.present();
    }

    pub fn sleep(&self, ms: u64) {
        let duration = time::Duration::from_millis(ms);
        thread::sleep(duration);
    }

    pub fn get_timestamp(&self) -> u64 {
        (time::Instant::now().elapsed().as_millis() & std::u64::MAX as u128) as u64
    }

    pub fn start_audio(&mut self, audio: Arc<RwLock<mixer::Mixer>>) {
        debug!("Starting audio");
        let audio_subsystem = self.sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(22050),
            channels: Some(1),
            samples: None,
        };

        let device = audio_subsystem
            .open_playback(None, &desired_spec, |spec| {
                debug!("Actual spec: {:?}", spec);
                mixer::MixerAudio(audio)
            })
            .unwrap();

        device.resume();
        self.audio_device = Some(device);
    }
}
