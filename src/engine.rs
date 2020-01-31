use crate::parts;
use crate::vm::VirtualMachine;

pub struct Engine {
    vm: VirtualMachine,
}

impl Engine {
    pub fn new(mut vm: VirtualMachine) -> Engine {
        vm.init_for_part(parts::GAME_PART_FIRST);
        Engine { vm }
    }

    pub fn run(&mut self) {
        self.vm.check_thread_requests();
        self.vm.host_frame();
    }
}
