use log::debug;
use std::sync::{Arc, RwLock};
use std::{thread, time};

use sdl2::audio::{AudioDevice, AudioSpecDesired};
use sdl2::pixels::{Color, Palette, PixelFormatEnum};
use sdl2::render::{Texture, TextureCreator, WindowCanvas};
use sdl2::surface::Surface;
use sdl2::video::WindowContext;

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
    texture_creator: TextureCreator<WindowContext>,
    scanlines: bool,
    scanline_overlay_size: (u32, u32),
    scanline_texture: Option<Texture>,
}

fn create_scanline_overlay(display_width: u32, display_height: u32) -> Surface<'static> {
    let mut surface = Surface::new(display_width, display_height, PixelFormatEnum::RGBA8888).unwrap();

    let val = 48;
    let step = display_height as usize / 200;
    if step < 3 {
        return surface;
    }
    surface.with_lock_mut(|p| {
        for j in (1..display_height).step_by(step) {
            for i in 0..display_width {
                p[(((j-1)*display_width*4)+i*4) as usize] = val;
                p[((j*display_width*4)+i*4) as usize] = val;
            }
        }
    });
    surface
}

impl SDLSys {
    pub fn new(sdl_context: sdl2::Sdl, width: usize, height: usize, scanlines: bool) -> SDLSys {
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

        let texture_creator = canvas.texture_creator();

        SDLSys {
            sdl_context,
            surface: Surface::new(width as u32, height as u32, PixelFormatEnum::Index8).unwrap(),
            canvas,
            audio_device: None,
            timestamp: time::Instant::now(),
            width,
            height,
            texture_creator,
            scanlines,
            scanline_overlay_size: (0, 0),
            scanline_texture: None,
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
        let texture = self.texture_creator
            .create_texture_from_surface(&*self.surface)
            .unwrap();

        self.canvas.clear();
        self.canvas
            .copy(&texture, None, None)
            .unwrap();

        if self.scanlines && self.scanline_overlay_size != self.canvas.output_size().unwrap() {
            let (display_width, display_height) = self.canvas.output_size().unwrap();
            let scanline_overlay = create_scanline_overlay(display_width, display_height);
            let overlay = self.texture_creator
                .create_texture_from_surface(&*scanline_overlay)
                .unwrap();
            self.scanline_texture = Some(overlay);
            self.scanline_overlay_size = (display_width, display_height);
        }

        if let Some(scanline_texture) = &self.scanline_texture {
            self.canvas
                .copy(&scanline_texture, None, None)
                .unwrap();
        }

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
