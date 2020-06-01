use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, RwLock};

use chrono;
use log::{debug, trace};
use timer::{Guard, Timer};

use crate::buffer::Buffer;
use crate::mixer::{MixerAudio, MixerChunk};

pub struct SfxInstrument {
    data: Vec<u8>,
    volume: u16,
}

impl SfxInstrument {
    pub fn new(data: Vec<u8>, volume: u16) -> SfxInstrument {
        SfxInstrument { data, volume }
    }
}

pub struct SfxModule {
    data: Vec<u8>,
    cur_pos: usize,
    cur_order: u8,
    num_order: u8,
    order_table: [u8; 0x80],
    samples: Vec<Option<SfxInstrument>>,
}

impl SfxModule {
    pub fn new(
        data: Vec<u8>,
        cur_order: u8,
        num_order: u8,
        order_table: [u8; 0x80],
        samples: Vec<Option<SfxInstrument>>,
    ) -> SfxModule {
        SfxModule {
            data,
            cur_pos: 0,
            cur_order,
            num_order,
            order_table,
            samples,
        }
    }
}

pub enum PatternResult {
    StopChannel(u8),
    MarkVariable(u16),
    Pattern(u8, SfxPattern),
}

pub struct SfxPattern {
    pub note1: u16,
    pub note2: u16,
    pub sample_buffer: Vec<u8>,
    pub sample_len: usize,
    pub loop_pos: usize,
    pub loop_len: usize,
    pub sample_volume: u16,
}

impl SfxPattern {
    fn from_notes(note1: u16, note2: u16, sample: &SfxInstrument) -> SfxPattern {
        let mut buffer = Buffer::new(&sample.data);
        let sample_len = (buffer.fetch_word() * 2) as usize;
        let loop_len = (buffer.fetch_word() * 2) as usize;
        let (loop_pos, loop_len) = if loop_len != 0 {
            (sample_len, loop_len)
        } else {
            (0, 0)
        };

        let mut m = sample.volume;
        let effect = (note2 & 0x0f00) >> 8;
        let volume = note2 & 0xff;
        if effect == 5 {
            // volume up
            m += volume;
            if m > 0x3f {
                m = 0x3f;
            }
        } else if effect == 6 {
            // volume down;
            if m < volume {
                m = 0;
            } else {
                m -= volume;
            }
        }
        let sample_start = 8;
        SfxPattern {
            note1,
            note2,
            sample_buffer: sample.data[sample_start..].to_vec(),
            sample_len,
            loop_pos,
            loop_len,
            sample_volume: m,
        }
    }
}

pub struct SfxPlayer {
    delay: i64,
    sfx_module: Option<SfxModule>,
    timer: Timer,
    timer_guard: Option<Guard>,
}

impl SfxPlayer {
    pub fn new() -> SfxPlayer {
        SfxPlayer {
            delay: 0,
            sfx_module: None,
            timer: Timer::new(),
            timer_guard: None,
        }
    }

    pub fn set_events_delay(&mut self, delay: u16) {
        debug!("set_events_delay({})", delay);
        self.delay = (delay as u32 * 60 / 7050) as i64;
    }

    pub fn set_sfx_module(&mut self, module: SfxModule) {
        trace!("Setting sfx module");
        self.sfx_module = Some(module);
    }

    pub fn start(&mut self, mixer: MixerAudio) -> Receiver<i16> {
        let (tx, rx) = sync_channel::<i16>(0);
        if let Some(sfx_module) = self.sfx_module.take() {
            let sfx_module = Arc::new(RwLock::new(sfx_module));
            self.timer_guard.replace(self.timer.schedule_repeating(
                chrono::Duration::milliseconds(self.delay),
                move || {
                    if let Some(variable) =
                        SfxPlayer::handle_events(sfx_module.clone(), mixer.clone())
                    {
                        tx.send(variable).unwrap();
                    }
                },
            ));
        }
        rx
    }

    pub fn stop(&mut self) {
        self.timer_guard.take();
    }

    pub fn handle_events(sfx_module: Arc<RwLock<SfxModule>>, mixer: MixerAudio) -> Option<i16> {
        let mut variable_value = None;

        let mut sfx_module = sfx_module.write().expect("Expected non-poisoned RwLock");
        let order = sfx_module.order_table[sfx_module.cur_order as usize] as usize;
        let mut mixer_guard = mixer.0.write().expect("Expected non-poisoned RwLock");
        for ch in 0..4 {
            let start = sfx_module.cur_pos + order * 1024 + ch * 4;
            trace!("Start: {}", start);
            let pattern_data = Buffer::new(&sfx_module.data[start..start + 4]);
            let result = SfxPlayer::handle_pattern(&sfx_module, ch as u8, pattern_data);
            match result {
                Some(PatternResult::StopChannel(channel)) => mixer_guard.stop_channel(channel),
                Some(PatternResult::MarkVariable(var)) => variable_value = Some(var as i16),
                Some(PatternResult::Pattern(channel, pat)) => {
                    trace!("Playing music");
                    assert!(pat.note1 >= 0x37);
                    assert!(pat.note1 < 0x1000);
                    let freq = (7_159_092 / (pat.note1 * 2) as u32) as u16;
                    let volume = pat.sample_volume;
                    let chunk = MixerChunk::from_sfx_pattern(pat);
                    mixer_guard.play_channel(channel, chunk, freq, volume as u8);
                }
                None => {}
            }
        }

        let order = sfx_module.order_table[sfx_module.cur_order as usize] as usize;
        sfx_module.cur_pos += 4 * 4;
        debug!(
            "handle_events() order = 0x{:x} cur_pos = 0x{:x}",
            order, sfx_module.cur_pos
        );
        if sfx_module.cur_pos >= 1024 {
            sfx_module.cur_pos = 0;
            let order = sfx_module.cur_order + 1;
            if order == sfx_module.num_order {
                //STOP PLAYING
            }
            sfx_module.cur_order = order;
        }
        variable_value
    }

    fn handle_pattern(
        sfx_module: &SfxModule,
        channel: u8,
        mut pattern_data: Buffer,
    ) -> Option<PatternResult> {
        let note1 = pattern_data.fetch_word();
        let note2 = pattern_data.fetch_word();
        trace!("Note1: {}, Note2: {}", note1, note2);
        if note1 != 0xfffd {
            if note1 == 0xfffe {
                trace!("Stop channel {}", channel);
                return Some(PatternResult::StopChannel(channel));
            }
            let sample_index = ((note2 & 0xf000) >> 12) as usize;
            if sample_index != 0 {
                trace!("Have sample index");
                if let Some(sample) = sfx_module.samples[sample_index - 1].as_ref() {
                    trace!("Sample len: {}", sample.data.len());
                    return Some(PatternResult::Pattern(
                        channel,
                        SfxPattern::from_notes(note1, note2, &sample),
                    ));
                }
            }
        } else {
            return Some(PatternResult::MarkVariable(note2));
        }
        None
    }
}
