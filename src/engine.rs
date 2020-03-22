use crate::parts;
use crate::vm::VirtualMachine;

pub struct Engine {
    vm: VirtualMachine,
}

impl Engine {
    #[cfg(feature = "bypass_protection")]
    pub fn new(mut vm: VirtualMachine) -> Engine {
        vm.init_for_part(parts::GAME_PART2);
        Engine { vm }
    }

    #[cfg(not(feature = "bypass_protection"))]
    pub fn new(mut vm: VirtualMachine) -> Engine {
        vm.init_for_part(parts::GAME_PART1);
        Engine { vm }
    }

    pub fn run(&mut self) {
        loop {
            self.vm.check_thread_requests();
            if !self.vm.update_player_input() {
                return;
            }
            self.vm.host_frame();
        }
    }
}
