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

pub struct SDLSys {
    sdl_context: sdl2::Sdl,
    surface: Surface<'static>,
    canvas: WindowCanvas,
    audio_device: Option<AudioDevice<mixer::MixerAudio>>,
    timestamp: time::Instant,
    width: usize,
    height: usize,
}

impl SDLSys {
    pub fn new(sdl_context: sdl2::Sdl, width: usize, height: usize) -> SDLSys {
        let video_subsystem = sdl_context.video().unwrap();

        let window = video_subsystem
            .window("Another world", 1280, 800)
            .position_centered()
            .resizable()
            .build()
            .unwrap();

        let mut canvas = window.into_canvas().build().expect("Expected canvas");
        canvas
            .set_logical_size(width as u32, height as u32)
            .expect("Expected logical size");
        SDLSys {
            sdl_context,
            surface: Surface::new(width as u32, height as u32, PixelFormatEnum::Index8).unwrap(),
            canvas,
            audio_device: None,
            timestamp: time::Instant::now(),
            width,
            height,
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
        let width = self.width;
        let height = self.height;
        self.surface.with_lock_mut(|p| {
            for j in 0..height {
                let p_offset = pitch * j;
                let page_offset = j * width;
                p[p_offset..(width + p_offset)]
                    .clone_from_slice(&page.data[page_offset..(width + page_offset)]);
            }
        });
        let texture_creator = self.canvas.texture_creator();
        let texture = texture_creator
            .create_texture_from_surface(&*self.surface)
            .unwrap();
        self.canvas.clear();
        self.canvas
            .copy(
                &texture,
                None,
                Some(Rect::new(0, 0, width as u32, height as u32)),
            )
            .unwrap();
        self.canvas.present();
    }

    pub fn sleep(&self, ms: u64) {
        let duration = time::Duration::from_millis(ms);
        thread::sleep(duration);
    }

    pub fn get_timestamp(&self) -> u64 {
        (self.timestamp.elapsed().as_millis() & std::u64::MAX as u128) as u64
    }

    pub fn start_audio(&mut self, audio: Arc<RwLock<mixer::Mixer>>) {
        debug!("Starting audio");
        let audio_subsystem = self.sdl_context.audio().unwrap();

        let desired_spec = AudioSpecDesired {
            freq: Some(mixer::SOUND_SAMPLE_RATE as i32),
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
