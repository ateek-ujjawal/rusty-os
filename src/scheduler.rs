// Scheduler for processes

use crate::process::{ProcessState, PROCESS_LIST};

// Takes a process from the front of the process list
// and returns it's trap frame, program counter and the satp(for the root page table)
pub fn schedule() -> (usize, usize, usize) {
    unsafe {
        if let Some(mut pl) = PROCESS_LIST.take() {
            pl.rotate_left(1);
            let mut frame_addr = 0;
            let mut mepc = 0;
            let mut pid = 0;
            let mut satp_root = 0;

            if let Some(process) = pl.front() {
                match process.get_state() {
                    ProcessState::Running => {
                        frame_addr = process.get_frame_address();
                        mepc = process.get_program_counter();
                        pid = process.get_pid() as usize;
                        satp_root = process.get_table_address() >> 12;
                    },
                    ProcessState::Sleeping => {

                    }
                    _ => {},
                }
            }
            println!("Scheduling {}", pid);
            PROCESS_LIST.replace(pl);
            if frame_addr != 0 {
                if satp_root != 0 {
                    return (frame_addr, mepc, (8 << 60) | (pid << 44) | (satp_root));
                } else {
                    return (frame_addr, mepc, 0);
                }
            }
        }
        (0, 0, 0)
    }
}