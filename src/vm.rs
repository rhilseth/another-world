use log::debug;

use crate::buffer::Buffer;
use crate::opcode::Opcode;
use crate::resource::Resource;
use crate::video::{Point, Video};

const NUM_VARIABLES: usize = 256;
const NUM_THREADS: usize = 64;
const SET_INACTIVE_THREAD: usize = 0xfffe;
const INACTIVE_THREAD: usize = 0xffff;
const COLOR_BLACK: u8 = 0xff;
const DEFAULT_ZOOM: u16 = 0x40;

#[derive(Copy, Clone)]
struct Thread {
    script_stack_call: usize,
    pc: usize,
    requested_pc_offset: Option<usize>,
    is_channel_active_current: bool,
    is_channel_active_requested: bool,
}

impl Thread {
    fn new() -> Thread {
        Thread {
            script_stack_call: 0,
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
}

impl VirtualMachine {
    pub fn new(resource: Resource, video: Video) -> VirtualMachine {
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
            let opcode = Opcode::decode(self.fetch_byte());

            match opcode {
                Opcode::DrawString => self.draw_string(),
                Opcode::DrawPolyBackground(val) => self.draw_poly_background(val),
                val => unimplemented!("Unimplemented opcode: {:?}", val),
            }
        }
    }

    fn draw_string(&mut self) {
        let string_id = self.fetch_word();
        let x = self.fetch_byte() as u16;
        let y = self.fetch_byte() as u16;
        let color = self.fetch_byte() as u16;
        self.video.draw_string(color, x, y, string_id);
    }

    fn draw_poly_background(&mut self, val: u8) {
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
