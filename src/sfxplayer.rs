use std::time;

use log::debug;

use crate::buffer::Buffer;

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
    delay: u16,
    sfx_module: Option<SfxModule>,
    timestamp: time::Instant,
    last_timestamp: u128,
}

impl SfxPlayer {
    pub fn new() -> SfxPlayer {
        SfxPlayer {
            delay: 0,
            sfx_module: None,
            timestamp: time::Instant::now(),
            last_timestamp: 0,
        }
    }

    pub fn set_events_delay(&mut self, delay: u16) {
        debug!("set_events_delay({})", delay);
        self.delay = (delay as u32 * 60 / 7050) as u16;
    }

    pub fn set_sfx_module(&mut self, module: SfxModule) {
        self.sfx_module = Some(module);
    }

    pub fn delay(&self) -> u16 {
        self.delay
    }

    pub fn handle_events(&mut self) -> Option<PatternResult> {
        let mut result = None;
        let ts = self.timestamp.elapsed().as_millis();
        let since_last_call = ts - self.last_timestamp;
        debug!("handle_events() {}", since_last_call);
        self.last_timestamp = ts;

        if let Some(sfx_module) = &self.sfx_module {
            let order = sfx_module.order_table[sfx_module.cur_order as usize] as usize;
            for ch in 0..4 {
                let start = sfx_module.cur_pos + order * 1024 + ch * 4;
                let pattern_data = Buffer::new(&sfx_module.data[start..start + 4]);
                result = self.handle_pattern(ch as u8, pattern_data);
            }
        }

        if let Some(sfx_module) = &mut self.sfx_module {
            let order = sfx_module.order_table[sfx_module.cur_order as usize] as usize;
            sfx_module.cur_pos += 4 * 4;
            debug!("handle_events() order = 0x{:x} cur_pos = 0x{:x}", order, sfx_module.cur_pos);
            if sfx_module.cur_pos >= 1024 {
                sfx_module.cur_pos = 0;
                let order = sfx_module.cur_order + 1;
                if order == sfx_module.num_order {
                    //STOP PLAYING
                }
            }
        }
        result
    }

    fn handle_pattern(
        &self,
        channel: u8,
        mut pattern_data: Buffer
    ) -> Option<PatternResult> {
        let note1 = pattern_data.fetch_word();
        let note2 = pattern_data.fetch_word();
        if note1 != 0xfffd {
            if note1 == 0xfffe {
                return Some(PatternResult::StopChannel(channel));
            }
            let sample_index = ((note2 & 0xf000) >> 12) as usize;
            if sample_index != 0 {
                let sfx_module = self.sfx_module.as_ref()
                    .expect("sfx_module should be available here");
                let sample = sfx_module.samples[sample_index - 1].as_ref()
                    .expect("Expected some sample");
                return Some(
                    PatternResult::Pattern(
                        channel,
                        SfxPattern::from_notes(note1, note2, &sample)
                    )
                );
            }
        } else {
            return Some(PatternResult::MarkVariable(note2));
        }
        None
    }
}
