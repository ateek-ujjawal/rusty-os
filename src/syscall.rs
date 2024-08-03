// System calls

use crate::cpu::TrapFrame;

pub fn do_syscall(mepc: usize, frame: *mut TrapFrame) -> usize {
    let syscall_no;
    unsafe {
        // x10 register is a0, we get syscall number in a0 register
        syscall_no = (*frame).regs[10];
    }
    match syscall_no {
        0 => {
            // Exit syscall
            mepc + 4
        },
        1 => {
            println!("Test sycall");
            mepc + 4
        },
        _ => {
            println!("Unknown syscall number {}", syscall_no);
            mepc + 4
        }
    }
}