// CPU helper functions
// and kernel trap frame

use core::{arch::asm, ptr::null_mut};

#[repr(usize)]
pub enum SatpMode {
    Off = 0,
    Sv39 = 8,
    Sv48 = 9
}

// Trap frame structure
// This will be copied into the assembly code to handle the trap
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
    pub regs: [usize; 32], // 32 general purpose registers of 8 bytes each = 0 - 255
    pub fregs: [usize; 32], // 32 floating point registers of 8 bytes each = 255 - 511
    pub satp: usize,        // SATP Register 512 - 519
    pub trap_stack: *mut u8, // Trap stack to handle 520
    pub hartid: usize      // The current hart id 528
}

impl TrapFrame {
    // Zero out the trap frame
    pub const fn zero() -> Self {
        TrapFrame {
            regs: [0; 32],
            fregs: [0; 32],
            satp: 0,
            trap_stack: null_mut(),
            hartid: 0
        }
    }
}

pub static mut KERNEL_TRAP_FRAME: [TrapFrame; 8] = [TrapFrame::zero(); 8];

pub const fn build_satp(mode: SatpMode, asid: usize, addr: usize) -> usize {
    (mode as usize) << 60 | (asid & 0xffff) << 44 | (addr >> 12) & 0xff_ffff_ffff
}

pub fn mhartid_read() -> usize {
    unsafe {
        let hartid;
        asm!("csrr {}, mhartid", out(reg) hartid);
        hartid
    }
}

pub fn mstatus_write(val: usize) {
    unsafe {
        asm!("csrw mstatus, {}", in(reg) val);
    }
}

pub fn mstatus_read() -> usize {
	unsafe {
		let mstatus;
		asm!("csrr {}, mstatus", out(reg) mstatus);
		mstatus
	}
}

pub fn stvec_write(val: usize) {
	unsafe {
		asm!("csrw	stvec, {}", in(reg) val);
	}
}

pub fn stvec_read() -> usize {
	unsafe {
		let stvec;
		asm!("csrr	{}, stvec", out(reg) stvec);
		stvec
	}
}

pub fn mscratch_write(val: usize) {
	unsafe {
		asm!("csrw	mscratch, {}", in(reg) val);
	}
}

pub fn mscratch_read() -> usize {
	unsafe {
		let mscratch;
		asm!("csrr	{}, mscratch", out(reg) mscratch);
		mscratch
	}
}

pub fn mscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		asm!("csrrw	{0}, mscratch, {1}", out(reg) from, in(reg) to);
		from
	}
}

pub fn sscratch_write(val: usize) {
	unsafe {
		asm!("csrw	sscratch, {}", in(reg) val);
	}
}

pub fn sscratch_read() -> usize {
	unsafe {
		let sscratch;
		asm!("csrr	{}, sscratch", out(reg) sscratch);
		sscratch
	}
}

pub fn sscratch_swap(to: usize) -> usize {
	unsafe {
		let from;
		asm!("csrrw	{0}, sscratch, {1}", out(reg) from, in(reg) to);
		from
	}
}

pub fn sepc_write(val: usize) {
	unsafe {
		asm!("csrw sepc, {}", in(reg) val);
	}
}

pub fn sepc_read() -> usize {
	unsafe {
		let sepc;
		asm!("csrr {}, sepc", out(reg) sepc);
		sepc
	}
}

pub fn satp_write(val: usize) {
	unsafe {
		asm!("csrw satp, {}", in(reg) val);
	}
}

pub fn satp_read() -> usize {
	unsafe {
		let satp;
		asm!("csrr {}, satp", out(reg) satp);
		satp
	}
}

pub fn satp_fence(vaddr: usize, asid: usize) {
	unsafe {
		asm!("sfence.vma {0}, {1}", in(reg) vaddr, in(reg) asid);
	}
}

pub fn satp_fence_asid(asid: usize) {
	unsafe {
		asm!("sfence.vma zero, {}", in(reg) asid);
	}
}