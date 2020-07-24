use log::{debug, trace, warn};
use rand::random;
use std::cmp;
use std::io::Cursor;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};

use crate::mixer;
use crate::mixer::{Mixer, MixerAudio, MixerChunk};
use crate::opcode::Opcode;
use crate::parts;
use crate::player::PlayerDirection;
use crate::resource::Resource;
use crate::sfxplayer::SfxPlayer;
use crate::sys::SDLSys;
use crate::util;
use crate::video::{Palette, Point, Video};

const NUM_VARIABLES: usize = 256;
const NUM_THREADS: usize = 64;
const SET_INACTIVE_THREAD: usize = 0xfffe;
const INACTIVE_THREAD: usize = 0xffff;
const COLOR_BLACK: u8 = 0xff;
const DEFAULT_ZOOM: u32 = 0x40;
const STACK_SIZE: usize = 0xff;

const VM_VARIABLE_RANDOM_SEED: usize = 0x3c;
const VM_VARIABLE_LAST_KEYCHAR: usize = 0xda;
const VM_VARIABLE_HERO_POS_UP_DOWN: usize = 0xe5;
const VM_VARIABLE_MUS_MARK: usize = 0xf4;
const VM_VARIABLE_SCROLL_Y: usize = 0xf9;
const VM_VARIABLE_HERO_ACTION: usize = 0xfa;
const VM_VARIABLE_HERO_POS_JUMP_DOWN: usize = 0xfb;
const VM_VARIABLE_HERO_POS_LEFT_RIGHT: usize = 0xfc;
const VM_VARIABLE_HERO_POS_MASK: usize = 0xfd;
const VM_VARIABLE_HERO_ACTION_POS_MASK: usize = 0xfe;
const VM_VARIABLE_PAUSE_SLICES: usize = 0xff;

#[derive(Copy, Clone)]
struct Thread {
    pc: usize,
    requested_pc_offset: Option<usize>,
    is_channel_active_current: bool,
    is_channel_active_requested: bool,
}

impl Thread {
    fn new() -> Thread {
        Thread {
            pc: INACTIVE_THREAD,
            requested_pc_offset: None,
            is_channel_active_current: false,
            is_channel_active_requested: false,
        }
    }
}

pub enum VideoBufferSeg {
    Cinematic,
    Video2,
}

pub struct VirtualMachine {
    variables: [i16; NUM_VARIABLES],
    threads: [Thread; NUM_THREADS],
    mixer: Arc<RwLock<Mixer>>,
    resource: Resource,
    video: Video,
    player: SfxPlayer,
    requested_next_part: Option<u16>,
    script_ptr: usize,
    stack_ptr: usize,
    goto_next_thread: bool,
    video_buffer_seg: VideoBufferSeg,
    script_stack_calls: [usize; STACK_SIZE],
    sys: SDLSys,
    last_timestamp: u64,
    variable_receiver: Option<Receiver<i16>>,
    scale: u32,
}

impl VirtualMachine {
    pub fn new(resource: Resource, video: Video, mut sys: SDLSys, scale: u32) -> VirtualMachine {
        let mut variables = [0; NUM_VARIABLES];
        variables[0x54] = 0x81;
        variables[VM_VARIABLE_RANDOM_SEED] = random::<i16>();
        let mixer = Arc::new(RwLock::new(Mixer::new()));
        sys.start_audio(mixer.clone());
        VirtualMachine {
            variables,
            threads: [Thread::new(); NUM_THREADS],
            mixer,
            resource,
            video,
            player: SfxPlayer::new(),
            requested_next_part: None,
            script_ptr: 0,
            stack_ptr: 0,
            goto_next_thread: false,
            video_buffer_seg: VideoBufferSeg::Cinematic,
            script_stack_calls: [0; STACK_SIZE],
            sys,
            last_timestamp: 0,
            variable_receiver: None,
            scale,
        }
    }

    pub fn set_variable(&mut self, var: usize, value: i16) {
        self.variables[var] = value;
    }

    pub fn init_for_part(&mut self, part_id: u16) {
        debug!("init_for_part: {}", part_id);
        self.player.stop();
        self.mixer
            .write()
            .expect("Expected non-poisoned RwLock")
            .stop_all();

        self.variables[0xe4] = 0x14;

        self.resource.setup_part(part_id);
        if self.resource.copy_vid_ptr {
            let mut video_page_data = self.resource.video_page_data();
            debug!("init_for_part copy_vid_ptr: {}", video_page_data.len());
            if self.scale != 1 {
                video_page_data = util::resize(&video_page_data, self.scale);
            }
            self.video.copy_page_buffer(&video_page_data);
            self.resource.copy_vid_ptr = false;
        }
        // copy

        for thread in self.threads.iter_mut() {
            thread.pc = 0xffff;
            thread.requested_pc_offset = None;
            thread.is_channel_active_current = false;
            thread.is_channel_active_requested = false;
        }

        self.threads[0].pc = 0;
    }

    pub fn check_thread_requests(&mut self) {
        // Check if a part switch has been requested
        if let Some(part) = self.requested_next_part {
            trace!("New part requested: {}", part);
            self.init_for_part(part);
            self.requested_next_part = None;
        }

        // Check if a PAUSE or JUMP has been requested
        for (thread_id, thread) in self.threads.iter_mut().enumerate() {
            let requested = thread.is_channel_active_requested;
            thread.is_channel_active_current = requested;

            if let Some(pc_offset) = thread.requested_pc_offset {
                thread.pc = if pc_offset == SET_INACTIVE_THREAD {
                    INACTIVE_THREAD
                } else {
                    pc_offset
                };
                thread.requested_pc_offset = None;
                trace!("Setting thread {} pc to 0x{:x}", thread_id, thread.pc);
            }
        }
    }

    pub fn update_player_input(&mut self) -> bool {
        let input = self.sys.process_events();

        if self.resource.current_part_id == 0x3e89 {
            let c = input.last_char;
            if c == '\x08' || c == '\0' || (c >= 'A' && c <= 'Z') {
                self.variables[VM_VARIABLE_LAST_KEYCHAR] = c as i16;
            }
        }

        if input.quit {
            return false;
        }

        if input.code
            && self.resource.current_part_id != parts::GAME_PART_LAST
            && self.resource.current_part_id != parts::GAME_PART_FIRST
        {
            self.requested_next_part = Some(parts::GAME_PART_LAST);
        }

        let mut lr = 0;
        let mut m = 0;
        let mut ud = 0;
        if input.direction.contains(PlayerDirection::RIGHT) {
            lr = 1;
            m |= 1;
        }
        if input.direction.contains(PlayerDirection::LEFT) {
            lr = -1;
            m |= 2;
        }
        if input.direction.contains(PlayerDirection::DOWN) {
            ud = 1;
            m |= 4;
        }
        if input.direction.contains(PlayerDirection::UP) {
            ud = -1;
            m |= 8;
        }
        self.variables[VM_VARIABLE_HERO_POS_UP_DOWN] = ud;
        self.variables[VM_VARIABLE_HERO_POS_JUMP_DOWN] = ud;
        self.variables[VM_VARIABLE_HERO_POS_LEFT_RIGHT] = lr;
        self.variables[VM_VARIABLE_HERO_POS_MASK] = m;

        self.variables[VM_VARIABLE_HERO_ACTION] = if input.button {
            m |= 0x80;
            1
        } else {
            0
        };
        self.variables[VM_VARIABLE_HERO_ACTION_POS_MASK] = m;
        true
    }

    pub fn host_frame(&mut self) {
        for thread_id in 0..self.threads.len() {
            if self.threads[thread_id].is_channel_active_current {
                trace!("Skip thread {}", thread_id);
                continue;
            }

            let n = self.threads[thread_id].pc;
            if n != INACTIVE_THREAD {
                //debug!("Start of bytecode: {}", self.resource.seg_bytecode);
                self.script_ptr = self.resource.seg_bytecode + n;
                self.stack_ptr = 0;
                self.goto_next_thread = false;

                debug!("host_frame() thread_id=0x{:02x} n=0x{:02x}", thread_id, n);

                self.execute_thread();

                // Save pc since it will be modified on the next iteration
                self.threads[thread_id].pc = self.script_ptr - self.resource.seg_bytecode;

                debug!(
                    "host_frame() thread_id=0x{:02x} pos=0x{:x}",
                    thread_id, self.threads[thread_id].pc
                );

                // if input.quit { break }....
            }
        }
    }

    fn fetch_byte(&mut self) -> u8 {
        let result = self.resource.read_byte(self.script_ptr);
        self.script_ptr += 1;
        result
    }

    fn fetch_word(&mut self) -> u16 {
        let result = self.resource.read_word(self.script_ptr);
        self.script_ptr += 2;
        result
    }

    fn execute_thread(&mut self) {
        while !self.goto_next_thread {
            if let Some(rx) = &self.variable_receiver {
                if let Ok(value) = rx.try_recv() {
                    debug!("Got variable value from sfxplayer: {}", value);
                    self.variables[VM_VARIABLE_MUS_MARK] = value;
                }
            }
            trace!("pc: 0x{:x} Decoding opcode", self.script_ptr);
            let opcode = Opcode::decode(self.fetch_byte());

            match opcode {
                Opcode::MovConst => self.op_mov_const(),
                Opcode::Mov => self.op_mov(),
                Opcode::Add => self.op_add(),
                Opcode::AddConst => self.op_add_const(),
                Opcode::Call => self.op_call(),
                Opcode::Ret => self.op_ret(),
                Opcode::PauseThread => self.op_pause_thread(),
                Opcode::Jmp => self.op_jmp(),
                Opcode::SetSetVect => self.op_set_set_vect(),
                Opcode::Jnz => self.op_jnz(),
                Opcode::CondJmp => self.op_cond_jmp(),
                Opcode::SetPalette => self.op_set_palette(),
                Opcode::ResetThread => self.op_reset_thread(),
                Opcode::SelectVideoPage => self.op_select_video_page(),
                Opcode::FillVideoPage => self.op_fill_video_page(),
                Opcode::CopyVideoPage => self.op_copy_video_page(),
                Opcode::BlitFrameBuffer => self.op_blit_frame_buffer(),
                Opcode::KillThread => self.op_kill_thread(),
                Opcode::DrawString => self.op_draw_string(),
                Opcode::Sub => self.op_sub(),
                Opcode::And => self.op_and(),
                Opcode::Or => self.op_or(),
                Opcode::Shl => self.op_shl(),
                Opcode::Shr => self.op_shr(),
                Opcode::PlaySound => self.op_play_sound(),
                Opcode::UpdateMemList => self.op_update_memlist(),
                Opcode::PlayMusic => self.op_play_music(),
                Opcode::DrawPolySprite(val) => self.op_draw_poly_sprite(val),
                Opcode::DrawPolyBackground(val) => self.op_draw_poly_background(val),
            }
        }
    }

    // Opcode implementation

    fn op_mov_const(&mut self) {
        let variable_id = self.fetch_byte() as usize;
        let value = self.fetch_word() as i16;
        debug!("mov_const(0x{:02x}, {})", variable_id, value);
        self.variables[variable_id] = value;
    }

    fn op_mov(&mut self) {
        let dst_variable_id = self.fetch_byte() as usize;
        let src_variable_id = self.fetch_byte() as usize;
        debug!("mov(0x{:02x}, 0x{:02x})", dst_variable_id, src_variable_id);
        self.variables[dst_variable_id] = self.variables[src_variable_id];
    }

    fn op_add(&mut self) {
        let dst_variable_id = self.fetch_byte() as usize;
        let src_variable_id = self.fetch_byte() as usize;
        debug!("add(0x{:02x}, 0x{:02x})", dst_variable_id, src_variable_id);
        self.variables[dst_variable_id] =
            self.variables[dst_variable_id].wrapping_add(self.variables[src_variable_id]);
    }

    fn op_add_const(&mut self) {
        // Insert gun sound hack here at some point
        let variable_id = self.fetch_byte() as usize;
        let value = self.fetch_word() as i16;
        debug!("add_const(0x{:02x}, {})", variable_id, value);
        self.variables[variable_id] = self.variables[variable_id].wrapping_add(value);
    }

    fn op_call(&mut self) {
        let offset = self.fetch_word();

        debug!("call(0x{:x})", offset);
        self.script_stack_calls[self.stack_ptr] = self.script_ptr - self.resource.seg_bytecode;
        if self.stack_ptr == STACK_SIZE {
            panic!("Stack overflow");
        }
        self.stack_ptr += 1;
        self.script_ptr = self.resource.seg_bytecode + offset as usize;
    }

    fn op_ret(&mut self) {
        debug!("ret()");
        if self.stack_ptr == 0 {
            panic!("Stack underflow!");
        }
        self.stack_ptr -= 1;
        self.script_ptr = self.resource.seg_bytecode + self.script_stack_calls[self.stack_ptr]
    }

    fn op_pause_thread(&mut self) {
        debug!("pause_thread()");
        self.goto_next_thread = true;
    }

    fn op_jmp(&mut self) {
        let pc_offset = self.fetch_word() as usize;
        debug!("op_jmp(0x{:02x})", pc_offset);
        self.script_ptr = self.resource.seg_bytecode + pc_offset;
    }

    fn op_set_set_vect(&mut self) {
        let thread_id = self.fetch_byte() as usize;
        let pc_offset_requested = self.fetch_word() as usize;
        debug!(
            "set_set_vect(0x{:02x}, 0x{:x})",
            thread_id, pc_offset_requested
        );
        self.threads[thread_id].requested_pc_offset = Some(pc_offset_requested);
    }

    fn op_jnz(&mut self) {
        let i = self.fetch_byte() as usize;
        debug!("jnz(0x{:02x})", i);
        self.variables[i] = self.variables[i].wrapping_sub(1);
        if self.variables[i] != 0 {
            self.op_jmp();
        } else {
            let _ = self.fetch_word();
        }
    }

    fn op_cond_jmp(&mut self) {
        let opcode = self.fetch_byte();
        let var = self.fetch_byte() as usize;
        let b = self.variables[var];

        let a = if opcode & 0x80 > 0 {
            let var = self.fetch_byte() as usize;
            self.variables[var]
        } else if opcode & 0x40 > 0 {
            self.fetch_word() as i16
        } else {
            self.fetch_byte() as i16
        };
        debug!(
            "op_cond_jmp({}, 0x{:02x}, 0x{:02x}) var=0x{:02x}",
            opcode, b, a, var
        );

        let expr = match opcode & 7 {
            0 => b == a,
            1 => b != a,
            2 => b > a,
            3 => b >= a,
            4 => b < a,
            5 => b <= a,
            _ => {
                warn!("op_cond_jmp() invalid condition {}", opcode & 7);
                false
            }
        };
        if expr {
            self.op_jmp();
        } else {
            self.fetch_word();
        }
    }

    fn op_set_palette(&mut self) {
        let palette_id = self.fetch_word();
        debug!("set_palette({})", palette_id);
        let palette_id = (palette_id >> 8) as u8;
        if palette_id >= 32 {
            return;
        }
        let palette_offset = palette_id as usize * 32;
        let start = self.resource.seg_palettes + palette_offset;
        let end = start + 32;
        let palette_data = &self.resource.memory[start..end];
        let palette = Palette::from_bytes(palette_data);
        self.video.palette_requested = Some(palette);
    }

    fn op_reset_thread(&mut self) {
        let thread_id = self.fetch_byte() as usize;
        let mut i = self.fetch_byte() as usize;

        i &= NUM_THREADS - 1;

        if i < thread_id {
            warn!("reset_thread() n < 0");
            return;
        }

        let n = i - thread_id + 1;
        let a = self.fetch_byte();

        debug!("reset_thread({}, {}, {})", thread_id, i, a);

        match a {
            0 | 1 => {
                let val = a != 0;
                for thread in thread_id..thread_id + n {
                    self.threads[thread].is_channel_active_requested = val;
                }
            }
            2 => {
                for thread in thread_id..thread_id + n {
                    self.threads[thread].requested_pc_offset = Some(SET_INACTIVE_THREAD);
                }
            }
            _ => {
                panic!("reset_thread() Invalid value for a {}", a);
            }
        }
    }

    fn op_select_video_page(&mut self) {
        let frame_buffer_id = self.fetch_byte();
        debug!("select_video_page({})", frame_buffer_id);
        self.video.change_page_ptr1(frame_buffer_id);
    }

    fn op_fill_video_page(&mut self) {
        let page_id = self.fetch_byte();
        let color = self.fetch_byte();
        debug!("fill_video_page({}, {})", page_id, color);
        self.video.fill_video_page(page_id, color);
    }

    fn op_copy_video_page(&mut self) {
        let src_page_id = self.fetch_byte();
        let dst_page_id = self.fetch_byte();
        debug!("copy_video_page({}, {})", src_page_id, dst_page_id);
        self.video.copy_page(
            src_page_id,
            dst_page_id,
            self.variables[VM_VARIABLE_SCROLL_Y],
        );
    }

    fn op_blit_frame_buffer(&mut self) {
        let page_id = self.fetch_byte();
        debug!("blit_frame_buffer({})", page_id);
        //inp_handle_special_keys();

        let delay = self.sys.get_timestamp() - self.last_timestamp;

        let pause_time = self.variables[VM_VARIABLE_PAUSE_SLICES] as u64 * 20;
        if pause_time > delay {
            let time_to_sleep = pause_time - delay;
            self.sys.sleep(time_to_sleep);
            trace!("Delay: {}, time_to_sleep: {}", delay, time_to_sleep);
        }
        self.last_timestamp = self.sys.get_timestamp();

        self.variables[0xf7] = 0;
        self.video.update_display(&mut self.sys, page_id);
    }

    fn op_kill_thread(&mut self) {
        debug!("kill_thread()");
        self.script_ptr = self.resource.seg_bytecode + 0xffff;
        self.goto_next_thread = true;
    }

    fn op_draw_string(&mut self) {
        let string_id = self.fetch_word();
        let x = self.fetch_byte() as u16;
        let y = self.fetch_byte() as u16;
        let color = self.fetch_byte();
        self.video.draw_string(color, x, y, string_id, self.scale);
    }

    fn op_sub(&mut self) {
        let i = self.fetch_byte() as usize;
        let j = self.fetch_byte() as usize;
        debug!("sub(0x{:02x}, 0x{:02x})", i, j);
        self.variables[i] = self.variables[i].wrapping_sub(self.variables[j]);
    }

    fn op_and(&mut self) {
        let variable_id = self.fetch_byte() as usize;
        let value = self.fetch_word() as i16;
        debug!("and(0x{:02x}, {}", variable_id, value);
        self.variables[variable_id] &= value;
    }

    fn op_or(&mut self) {
        let variable_id = self.fetch_byte() as usize;
        let value = self.fetch_word() as i16;
        debug!("or(0x{:02x}, {}", variable_id, value);
        self.variables[variable_id] |= value;
    }

    fn op_shl(&mut self) {
        let variable_id = self.fetch_byte() as usize;
        let left_shift = self.fetch_word();
        debug!("shl(0x{:02x}, {}", variable_id, left_shift);
        self.variables[variable_id] = ((self.variables[variable_id] as u16) << left_shift) as i16;
    }

    fn op_shr(&mut self) {
        let variable_id = self.fetch_byte() as usize;
        let right_shift = self.fetch_word();
        debug!("shl(0x{:02x}, {}", variable_id, right_shift);
        self.variables[variable_id] = ((self.variables[variable_id] as u16) >> right_shift) as i16;
    }

    fn op_play_sound(&mut self) {
        let resource_id = self.fetch_word();
        let freq = self.fetch_byte();
        let vol = self.fetch_byte();
        let channel = self.fetch_byte();
        debug!(
            "play_sound(0x{:x}, {}, {}, {})",
            resource_id, freq, vol, channel
        );
        self.play_sound_resource(resource_id, freq, vol, channel);
    }

    fn op_update_memlist(&mut self) {
        let resource_id = self.fetch_word();
        debug!("update_memlist({})", resource_id);

        if resource_id == 0 {
            self.player.stop();
            self.mixer
                .write()
                .expect("Expected non-poisoned RwLock")
                .stop_all();
            self.resource.invalidate_resource();
        } else if resource_id >= parts::GAME_PART_FIRST {
            debug!("Requesting new part {}", resource_id);
            self.requested_next_part = Some(resource_id);
        } else {
            self.resource.load_memory_entry(resource_id);
            if self.resource.copy_vid_ptr {
                let mut video_page_data = self.resource.video_page_data();
                debug!("update_memlist copy_vid_ptr: {}", video_page_data.len());
                if self.scale != 1 {
                    video_page_data = util::resize(&video_page_data, self.scale);
                }
                self.video.copy_page_buffer(&video_page_data);
                self.resource.copy_vid_ptr = false;
            }
        }
    }

    fn op_play_music(&mut self) {
        let resource_id = self.fetch_word();
        let delay = self.fetch_word();
        let pos = self.fetch_byte();
        self.play_music_resource(resource_id, delay, pos);
    }

    fn op_draw_poly_sprite(&mut self, val: u8) {
        let offset = (self.fetch_word() * 2) as usize;
        let mut x = self.fetch_byte() as i32;
        self.video_buffer_seg = VideoBufferSeg::Cinematic;

        if val & 0x20 == 0 {
            // bit 0010 0000
            if val & 0x10 == 0 {
                // bit 0001 000
                x = (x << 8) | self.fetch_byte() as i32;
            } else {
                x = self.variables[x as usize] as i32;
            }
        } else if val & 0x10 > 0 {
            // bit 0001 0000
            x += 0x100;
        }

        let mut y = self.fetch_byte() as i32;
        if val & 8 == 0 {
            // bit 0000 1000
            if val & 4 == 0 {
                // bit 0000 0100
                y = (y << 8) | self.fetch_byte() as i32;
            } else {
                y = self.variables[y as usize] as i32;
            }
        }

        let mut zoom = self.fetch_byte() as u32;

        if val & 2 == 0 {
            // bit 0000 0010
            if val & 1 == 0 {
                // bit 0000 0001
                self.script_ptr -= 1;
                zoom = 0x40;
            } else {
                zoom = self.variables[zoom as usize] as u32;
            }
        } else if val & 1 > 0 {
            // bit 0000 0001
            self.video_buffer_seg = VideoBufferSeg::Video2;
            self.script_ptr -= 1;
            zoom = 0x40;
        }
        debug!(
            "draw_poly_sprite() offset=0x{:x}, x={}, y={}, zoom={}",
            offset, x, y, zoom
        );
        let segment = match self.video_buffer_seg {
            VideoBufferSeg::Cinematic => self.resource.seg_cinematic,
            VideoBufferSeg::Video2 => self.resource.seg_video2,
        };
        let mut buffer = Cursor::new(&self.resource.memory[segment..]);
		buffer.set_position(offset as u64);
        let color = 0xff;
        let scale = self.scale as i32;
        let point = Point {
            x: x * scale,
            y: y * scale,
        };
        self.video
            .read_and_draw_polygon(&mut buffer, color, zoom * self.scale, point);
    }

    fn op_draw_poly_background(&mut self, val: u8) {
        let lsb = self.fetch_byte() as u16;

        // Avoid overflow when calculating offset by removing the
        // most significant bit
        let msb = (val & 0x7f) as u16;
        let offset: usize = (((msb << 8) | lsb) * 2) as usize;
        self.video_buffer_seg = VideoBufferSeg::Cinematic;

        let mut x = self.fetch_byte() as i32;
        let mut y = self.fetch_byte() as i32;
        let h = y - (self.video.height - 1) as i32;
        if h > 0 {
            y = self.video.height as i32 - 1;
            x += h;
        }
        debug!(
            "DrawPolyBackground: val: 0x{:02x} off={} x={} y={}",
            val, offset, x, y
        );

        let mut buffer =
            Cursor::new(&self.resource.memory[self.resource.seg_cinematic..]);
		buffer.set_position(offset as u64);
        let zoom = self.scale as i32;
        let point = Point {
            x: x * zoom,
            y: y * zoom,
        };
        self.video.read_and_draw_polygon(
            &mut buffer,
            COLOR_BLACK,
            DEFAULT_ZOOM * self.scale,
            point,
        );
    }

    fn stop_channel(&mut self, channel: u8) {
        let mut write_guard = self.mixer.write().expect("Expected non-poisoned RwLock");
        write_guard.stop_channel(channel);
    }

    fn play_channel(&mut self, channel: u8, mixer_chunk: MixerChunk, frequence: u16, vol: u8) {
        let mut write_guard = self.mixer.write().expect("Expected non-poisoned RwLock");
        let vol = cmp::min(vol, 0x3f);
        write_guard.play_channel(channel & 3, mixer_chunk, frequence, vol);
    }

    fn play_sound_resource(&mut self, resource_id: u16, freq: u8, vol: u8, channel: u8) {
        debug!(
            "play_sound_resource(0x{:x}, {}, {}, {})",
            resource_id, freq, vol, channel
        );
        if vol == 0 {
            self.stop_channel(channel);
        } else if let Some(mixer_chunk) = self.resource.get_entry_mixer_chunk(resource_id) {
            let frequence = mixer::FREQUENCE_TABLE[freq as usize];
            let vol = cmp::min(vol, 0x3f);
            self.play_channel(channel & 3, mixer_chunk, frequence, vol);
        }
    }

    fn play_music_resource(&mut self, resource_id: u16, delay: u16, pos: u8) {
        debug!(
            "play_music_resource(0x{:x}, {}, {})",
            resource_id, delay, pos
        );
        if resource_id != 0 {
            let mut delay = delay;
            if let Some(sfx_module) = self.resource.load_sfx_module(resource_id, &mut delay, pos) {
                self.player.set_sfx_module(sfx_module);
                self.player.set_events_delay(delay);

                self.variable_receiver
                    .replace(self.player.start(MixerAudio(self.mixer.clone())));
            }
        } else if delay != 0 {
            self.player.set_events_delay(delay);
        } else {
            self.player.stop();
        }
    }
}
