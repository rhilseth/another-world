use log::debug;

use crate::buffer::Buffer;
use crate::opcode::Opcode;
use crate::resource::Resource;
use crate::video::{Palette, Point, Video};
use crate::sys::SDLSys;

const NUM_VARIABLES: usize = 256;
const NUM_THREADS: usize = 64;
const SET_INACTIVE_THREAD: usize = 0xfffe;
const INACTIVE_THREAD: usize = 0xffff;
const COLOR_BLACK: u8 = 0xff;
const DEFAULT_ZOOM: u16 = 0x40;
const STACK_SIZE: usize = 0xff;

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
    resource: Resource,
    video: Video,
    requested_next_part: Option<u16>,
    script_ptr: usize,
    stack_ptr: usize,
    goto_next_thread: bool,
    video_buffer_seg: VideoBufferSeg,
    script_stack_calls: [usize; STACK_SIZE],
    sys: SDLSys,
    last_timestamp: u64,
}

impl VirtualMachine {
    pub fn new(resource: Resource, video: Video, sys: SDLSys) -> VirtualMachine {
        VirtualMachine {
            variables: [0; NUM_VARIABLES],
            threads: [Thread::new(); NUM_THREADS],
            resource,
            video,
            requested_next_part: None,
            script_ptr: 0,
            stack_ptr: 0,
            goto_next_thread: false,
            video_buffer_seg: VideoBufferSeg::Cinematic,
            script_stack_calls: [0; STACK_SIZE],
            sys,
            last_timestamp: 0,
        }
    }

    pub fn init_for_part(&mut self, part_id: u16) {
        debug!("init_for_part: {}", part_id);
        // player.stop();
        // mixer.stop_all();

        self.variables[0xe4] = 0x14;

        self.resource.setup_part(part_id);

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
            self.init_for_part(part);
            self.requested_next_part = None;
        }

        // Check if a PAUSE or JUMP has been requested
        for thread_id in 0..NUM_THREADS {
            let requested = self.threads[thread_id].is_channel_active_requested;
            self.threads[thread_id].is_channel_active_current = requested;

            if let Some(pc_offset) = self.threads[thread_id].requested_pc_offset {
                self.threads[thread_id].pc = if pc_offset == SET_INACTIVE_THREAD {
                    INACTIVE_THREAD
                } else {
                    pc_offset
                };
                self.threads[thread_id].requested_pc_offset = None;
            }
        }
    }

    pub fn host_frame(&mut self) {
        for thread_id in 0..self.threads.len() {
            if self.threads[thread_id].is_channel_active_current {
                continue;
            }

            let n = self.threads[thread_id].pc;
            if n != INACTIVE_THREAD {
                debug!("Start of bytecode: {}", self.resource.seg_bytecode);
                self.script_ptr = self.resource.seg_bytecode + n;
                self.stack_ptr = 0;
                self.goto_next_thread = false;

                debug!("host_frame() thread_id=0x{:02x} n=0x{:02x}", thread_id, n);

                self.execute_thread();

                // Save pc since it will be modified on the next iteration
                self.threads[thread_id].pc = self.script_ptr - self.resource.seg_bytecode;

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
            debug!("pc: 0x{:x} Decoding opcode", self.script_ptr);
            let opcode = Opcode::decode(self.fetch_byte());

            match opcode {
                Opcode::Call => self.op_call(),
                Opcode::Ret => self.op_ret(),
                Opcode::SetPalette => self.op_set_palette(),
                Opcode::SelectVideoPage => self.op_select_video_page(),
                Opcode::FillVideoPage => self.op_fill_video_page(),
                Opcode::BlitFrameBuffer => self.op_blit_frame_buffer(),
                Opcode::DrawString => self.op_draw_string(),
                Opcode::DrawPolyBackground(val) => self.op_draw_poly_background(val),
                val => unimplemented!("pc 0x{:x} Unimplemented opcode: {:?}", self.script_ptr, val),
            }
        }
    }

    // Opcode implementation

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

    fn op_blit_frame_buffer(&mut self) {
        let page_id = self.fetch_byte();
        debug!("blit_frame_buffer({})", page_id);
        //inp_handle_special_keys();

        let delay = self.sys.get_timestamp() - self.last_timestamp;
        let time_to_sleep = self.variables[VM_VARIABLE_PAUSE_SLICES] as u64 * 20 - delay;

        if time_to_sleep > 0 {
            self.sys.sleep(time_to_sleep);
        }

        self.last_timestamp = self.sys.get_timestamp();

        self.variables[0xf7] = 0;

        self.video.update_display(page_id);
    }

    fn op_draw_string(&mut self) {
        let string_id = self.fetch_word();
        let x = self.fetch_byte() as u16;
        let y = self.fetch_byte() as u16;
        let color = self.fetch_byte() as u16;
        self.video.draw_string(color, x, y, string_id);
    }

    fn op_draw_poly_background(&mut self, val: u8) {
        let lsb = self.fetch_byte() as u16;

        // Avoid overflow when calculating offset by removing the
        // most significant bit
        let msb = (val & 0x7f) as u16;
        let offset: usize = (((msb << 8) | lsb) * 2) as usize;
        self.video_buffer_seg = VideoBufferSeg::Cinematic;

        let mut x = self.fetch_byte() as i16;
        let mut y = self.fetch_byte() as i16;
        let h = y - 199;
        if h > 0 {
            y = 199;
            x += h;
        }
        debug!(
            "DrawPolyBackground: val: 0x{:02x} off={} x={} y={}",
            val, offset, x, y
        );

        let buffer = Buffer::with_offset(
            &self.resource.memory[self.resource.seg_cinematic..],
            offset
        );
        let point = Point { x, y };
        self.video.read_and_draw_polygon(buffer, COLOR_BLACK, DEFAULT_ZOOM, point);
    }
}
