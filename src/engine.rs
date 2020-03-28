use crate::parts;
use crate::vm::VirtualMachine;

pub struct Engine {
    vm: VirtualMachine,
}

impl Engine {
    pub fn new(mut vm: VirtualMachine, part_num: u8) -> Engine {
        let part = match part_num {
            1 => parts::GAME_PART1,
            2 => parts::GAME_PART2,
            3 => parts::GAME_PART3,
            4 => parts::GAME_PART4,
            5 => parts::GAME_PART5,
            6 => parts::GAME_PART6,
            7 => parts::GAME_PART7,
            8 => parts::GAME_PART8,
            9 => parts::GAME_PART9,
            10 => parts::GAME_PART10,
            n => panic!("Unknown part number: {}", n),
        };
        vm.init_for_part(part);
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
