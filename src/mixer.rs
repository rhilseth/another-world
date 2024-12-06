use std::sync::{Arc, RwLock};
use std::thread::sleep;
use std::time::Duration;

use log::{debug, trace};
use sdl2::audio::AudioCallback;

use crate::sfxplayer::SfxPattern;

pub const FREQUENCE_TABLE: [u16; 40] = [
    0x0CFF, 0x0DC3, 0x0E91, 0x0F6F, 0x1056, 0x114E, 0x1259, 0x136C, 0x149F, 0x15D9, 0x1726, 0x1888,
    0x19FD, 0x1B86, 0x1D21, 0x1EDE, 0x20AB, 0x229C, 0x24B3, 0x26D7, 0x293F, 0x2BB2, 0x2E4C, 0x3110,
    0x33FB, 0x370D, 0x3A43, 0x3DDF, 0x4157, 0x4538, 0x4998, 0x4DAE, 0x5240, 0x5764, 0x5C9A, 0x61C8,
    0x6793, 0x6E19, 0x7485, 0x7BBD,
];

const NUM_CHANNELS: usize = 4;

pub const SOUND_SAMPLE_RATE: u32 = 22050;

fn add_clamp(a: i16, b: i16) -> i8 {
    (a + b).clamp(-128, 127) as i8
}

pub struct MixerChunk {
    data: Vec<u8>,
    len: usize,
    loop_len: usize,
    loop_pos: usize,
}

impl MixerChunk {
    pub fn new(data: &[u8], len: usize, loop_len: usize) -> MixerChunk {
        let loop_pos = if loop_len > 0 { len } else { 0 };
        MixerChunk {
            data: data.to_vec(),
            len,
            loop_len,
            loop_pos,
        }
    }

    pub fn from_sfx_pattern(pattern: SfxPattern) -> MixerChunk {
        MixerChunk {
            data: pattern.sample_buffer,
            len: pattern.sample_len,
            loop_len: pattern.loop_len,
            loop_pos: pattern.loop_pos,
        }
    }
}

pub struct Mixer {
    channels: [Option<MixerChannel>; NUM_CHANNELS],
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer {
            channels: [None, None, None, None],
        }
    }

    pub fn play_channel(
        &mut self,
        channel: u8,
        mixer_chunk: MixerChunk,
        frequency: u16,
        volume: u8,
    ) {
        //debug!("mixer chunk {}, {}, {}", mixer_chunk.len, mixer_chunk.loop_len, mixer_chunk.loop_pos);
        self.channels[channel as usize] =
            Some(MixerChannel::new(volume, mixer_chunk, frequency.into()));
    }

    pub fn stop_channel(&mut self, channel: u8) {
        self.channels[channel as usize].take();
    }

    pub fn _set_channel_volume(&mut self, channel: u8, volume: u8) {
        if let Some(ref mut channel) = self.channels[channel as usize] {
            channel.volume = volume;
        }
    }

    pub fn stop_all(&mut self) {
        for channel in self.channels.iter_mut() {
            channel.take();
        }
    }
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct MixerAudio(pub Arc<RwLock<Mixer>>);

impl AudioCallback for MixerAudio {
    type Channel = i8;

    fn callback(&mut self, out: &mut [i8]) {
        trace!("MixerAudio::callback()");
        let mut write_guard = loop {
            if let Ok(write_guard) = self.0.write() {
                break write_guard;
            }
            sleep(Duration::from_millis(10));
        };
        for s in out.iter_mut() {
            *s = 0;
        }

        for (chan_num, ch) in write_guard.channels.iter_mut().enumerate() {
            if let Some(ref mut channel) = ch {
                for s in out.iter_mut() {
                    let ilc = (channel.chunk_pos & 0xff) as i16;
                    let p1 = channel.chunk_pos >> 8;
                    channel.chunk_pos += channel.chunk_inc;

                    let p2 = if channel.chunk.loop_len != 0 {
                        if p1 == channel.chunk.loop_pos + channel.chunk.loop_len - 1 {
                            debug!("Looping sample on channel {}", chan_num);
                            channel.chunk_pos = channel.chunk.loop_pos;
                            channel.chunk.loop_pos
                        } else {
                            p1 + 1
                        }
                    } else if channel.chunk.len == 0 || p1 == channel.chunk.len - 1 {
                        debug!("Stopping sample on channel {}", chan_num);
                        ch.take();
                        break;
                    } else {
                        p1 + 1
                    };
                    assert!(p1 < channel.chunk.data.len());
                    assert!(p2 < channel.chunk.data.len());
                    let b1 = channel.chunk.data[p1] as i8;
                    let b2 = channel.chunk.data[p2] as i8;
                    let b = ((b1 as i16 * (0xff - ilc) + b2 as i16 * ilc) >> 8) as i8;

                    *s = add_clamp(*s as i16, b as i16 * channel.volume as i16 / 0x40);
                    //debug!("j: {}, p1: {}, b1: {}, p2: {}, b2: {}, b: {}, sample: {}", j, p1, b1, p2, b2, b, *s);
                }
            }
        }
    }
}

struct MixerChannel {
    volume: u8,
    chunk: MixerChunk,
    chunk_pos: usize,
    chunk_inc: usize,
}

impl MixerChannel {
    pub fn new(volume: u8, chunk: MixerChunk, frequency: u32) -> MixerChannel {
        MixerChannel {
            volume,
            chunk,
            chunk_pos: 0,
            chunk_inc: ((frequency << 8) / SOUND_SAMPLE_RATE) as usize,
        }
    }
}
