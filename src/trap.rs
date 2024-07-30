// Trap handler

use crate::{cpu::TrapFrame, plic, uart};

#[no_mangle]
extern "C" fn m_trap(epc: usize, tval: usize, cause: usize, hart: usize, _status: usize, _frame: &TrapFrame) -> usize {
    // Check if trap is asynchronous(1) or synchronous(0)
    let is_async = if (cause >> 63) & 1 == 1 {
        true
    } else {
        false
    };

    // The mcause register holds the type of trap and the cause number
    // We get the last 12 bits of mcause to get the cause_num
    let cause_num = cause & 0xfff;
    let mut return_pc = epc;
    if is_async {
        match cause_num {
            3 => {
                // Machine software interrupt
                println!("Machine software interrupt CPU#{}", hart);
            },
            7 => {
                // Machine timer interrupt
                let mtimecmp = 0x0200_4000 as *mut u64;
				let mtime = 0x0200_bff8 as *const u64;
				// The frequency given by QEMU is 10_000_000 Hz, so this sets
				// the next interrupt to fire one second from now.
				unsafe { mtimecmp.write_volatile(mtime.read_volatile() + 10_000_000) };
            },
            11 => {
                // Machine external interrupt
                //println!("Machine external interrupt CPU#{}", hart);
				// Check id of next interrupt in claim register
				if let Some(interrupt) = plic::next() {
					match interrupt {
						10 => { 
							// Interrupt 10 is the UART interrupt.
							let mut my_uart = uart::Uart::new(0x1000_0000);
							if let Some(c) = my_uart.get() {
								match c {
									8 => {
										// This is a backspace, so we
										// essentially have to write a space and
										// backup again:
										print!("{} {}", 8 as char, 8 as char);
									},
									10 | 13 => {
										// Newline or carriage-return
										println!();
									},
									_ => {
										print!("{}", c as char);
									},
								}
							}
					
						},
						// Non-UART interrupts go here and do nothing.
						_ => {
							println!("Non-UART external interrupt: {}", interrupt);
						}
					}
					// We've claimed it, so now say that we've handled it. This resets the interrupt pending
					// and allows the UART to interrupt again.
					plic::complete(interrupt);
				}
            },
            _ => {
                println!("Unhandled async trap CPU#{} -> {}", hart, cause_num);
            }
        }
    } else {
        match cause_num {
			2 => {
				// Illegal instruction
				panic!("Illegal instruction CPU#{} -> 0x{:08x}: 0x{:08x}\n", hart, epc, tval);
			},
			8 => {
				// Environment (system) call from User mode
				println!("E-call from User mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc += 4;
			},
			9 => {
				// Environment (system) call from Supervisor mode
				println!("E-call from Supervisor mode! CPU#{} -> 0x{:08x}", hart, epc);
				return_pc += 4;
			},
			11 => {
				// Environment (system) call from Machine mode
				panic!("E-call from Machine mode! CPU#{} -> 0x{:08x}\n", hart, epc);
			},
			// Page faults
			12 => {
				// Instruction page fault
				println!("Instruction page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			13 => {
				// Load page fault
				println!("Load page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			15 => {
				// Store page fault
				println!("Store page fault CPU#{} -> 0x{:08x}: 0x{:08x}", hart, epc, tval);
				return_pc += 4;
			},
			_ => {
				panic!("Unhandled sync trap CPU#{} -> {}\n", hart, cause_num);
			}
        }
    }

    // Return updated program counter after printing/panicking on trap
    return_pc
}