use crate::resource::Resource;

const NUM_VARIABLES: usize = 256;
const NUM_THREADS: usize = 64;
const SET_INACTIVE_THREAD: usize = 0xfffe;
const INACTIVE_THREAD: usize = 0xffff;

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

pub struct VirtualMachine {
    variables: [i16; NUM_VARIABLES],
    threads: [Thread; NUM_THREADS],
    resource: Resource,
    requested_next_part: Option<u16>,
    script_ptr: usize,
    stack_ptr: usize,
}

impl VirtualMachine {
    pub fn new(resource: Resource) -> VirtualMachine {
        VirtualMachine {
            variables: [0; NUM_VARIABLES],
            threads: [Thread::new(); NUM_THREADS],
            resource,
            requested_next_part: None,
            script_ptr: 0,
            stack_ptr: 0,
        }
    }

    pub fn init_for_part(&mut self, part_id: u16) {
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
        for thread in self.threads.iter_mut() {
            if thread.is_channel_active_current {
                continue;
            }

            let n = thread.pc;
            if n != INACTIVE_THREAD {
                self.script_ptr = self.resource.seg_bytecode + n;
                self.stack_ptr = 0;
            }
        }
    }

}
