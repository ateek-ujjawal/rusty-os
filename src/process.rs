// Create and store processes

use alloc::collections::vec_deque::VecDeque;

use crate::{cpu::{build_satp, mscratch_write, satp_fence_asid, satp_write, SatpMode, TrapFrame},
            page::{alloc, dealloc, map, unmap, zalloc, EntryBits, Table, PAGE_SIZE}};

// Stack pages needed for each process
const STACK_PAGES: usize = 2;
// Stack virtual address that is seen by the user
const STACK_ADDR: usize = 0x1_0000_0000;
// All processes will have a defined starting point in virtual memory seen by the user.
const PROCESS_STARTING_ADDR: usize = 0x2000_0000;

// Here, we store a process list. It uses the global allocator
// that we made before and its job is to store all processes.
// We will have this list OWN the process. So, anytime we want
// the process, we will consult the process list.
// Using an Option here is one method of creating a "lazy static".
// Rust requires that all statics be initialized, but all
// initializations must be at compile-time. We cannot allocate
// a VecDeque at compile time, so we are somewhat forced to
// do this.
pub static mut PROCESS_LIST: Option<VecDeque<Process>> = None;
// We can search through the process list to get a new PID, but
// it's probably easier and faster just to increase the pid:
static mut NEXT_PID: u16 = 1;

// Gets make_syscall function symbol from trap.S file
extern "C" {
	fn make_syscall(a: usize) -> usize;
}

// We will eventually move this function out of here, but its
// job is just to take a slot in the process list.
fn init_process() {
	// We can't do much here until we have system calls because
	// we're running in User space.
    let mut i: usize = 0;
    loop {
        i += 1;
        if i > 70_000_000 {
            unsafe { 
                make_syscall(i);
            }
            i = 0;
        }
    }
}

// Add a process given a function address and then
// push it onto the LinkedList. Uses Process::new_default
// to create a new stack, etc.
pub fn add_process_default(pr: fn()) {
	unsafe {
		// PROCESS_LIST is wrapped in an Option<> enumeration, which
		// means that the Option owns the Deque. We can only borrow from
		// it or move ownership to us. In this case, we choose the
		// latter, where we move ownership to us, add a process, and
		// then move ownership back to the PROCESS_LIST.
		// This allows mutual exclusion as anyone else trying to grab
		// the process list will get None rather than the Deque.
		if let Some(mut pl) = PROCESS_LIST.take() {
			// .take() will replace PROCESS_LIST with None and give
			// us the only copy of the Deque.
			let p = Process::new_default(pr);
			pl.push_back(p);
			// Now, we no longer need the owned Deque, so we hand it
			// back by replacing the PROCESS_LIST's None with the
			// Some(pl).
			PROCESS_LIST.replace(pl);
		}
	}
}

// This should only be called once, and its job is to create
// the init process. Right now, this process is in the kernel,
// but later, it should call the shell.
pub fn init() -> usize {
	unsafe {
        // Initialize Process list with a deque(double ended queue with a capacity of 5 processes)
		PROCESS_LIST = Some(VecDeque::with_capacity(15));
        // Add the initial kernel process to the list and give it a process structure
		add_process_default(init_process);
        // We transfer ownership of the PROCESS_LIST to ourselves then give it back using replace
        // This ensures that any other process using the PROCESS_LIST does not interfere with it
		let pl = PROCESS_LIST.take().unwrap();
		let p = pl.front().unwrap().frame;
        // Get the program_counter address to jump to that function
        let func_vaddr = pl.front().unwrap().program_counter;
        // Take the trap frame of the process and write it to the mscratch
		let frame = p as *const TrapFrame as usize;
		mscratch_write(frame);
        // Fill the satp register with the root page table of the init process
		satp_write(build_satp(
			SatpMode::Sv39,
			1,
			pl.front().unwrap().root as usize,
		));
		// Synchronize PID 1. We use ASID as the PID.
		satp_fence_asid(1);
		// Put the process list back in the global.
		PROCESS_LIST.replace(pl);
		// Return the first instruction's address to execute from the program_counter variable
		func_vaddr
	}
}

// A process can have four states, represent them using an enum
pub enum ProcessState {
    Running,
    Sleeping,
    Waiting,
    Dead
}

// A process struct in C-style ABI
// A process includes the trap frame, it's stack, the program counter for execution, process id,
// root page table, process state and it's private data
#[repr(C)]
pub struct Process {
    frame:              *mut TrapFrame,
    stack:              *mut u8,
    program_counter:    usize,
    pid:                u16,
    root:               *mut Table,
    state:              ProcessState,
    data:               ProcessData,
    sleep_until:        usize
}

impl Process {
    pub fn get_frame_address(&self) -> usize {
        self.frame as usize
    }

    pub fn get_program_counter(&self) -> usize {
        self.program_counter as usize
    }

    pub fn get_pid(&self) -> u16 {
        self.pid
    }

    pub fn get_table_address(&self) -> usize {
        self.root as usize
    }

    pub fn get_state(&self) -> &ProcessState {
        &self.state
    }

    pub fn get_sleep_until(&self) -> usize {
        self.sleep_until as usize
    }

    // Create a new process with default conditions
    pub fn new_default(func: fn()) -> Self {
        let func_addr = func as usize;
        let func_vaddr = func_addr;
        let ret_proc = Process {
            frame:          zalloc(1) as *mut TrapFrame,
            stack:          alloc(STACK_PAGES),
            program_counter:PROCESS_STARTING_ADDR,
            pid:            unsafe { NEXT_PID },
            root:           zalloc(1) as *mut Table,
            state:          ProcessState::Running,
            data:           ProcessData::zero(),
            sleep_until:    0
        };
        unsafe { NEXT_PID += 1; }
        // Move stack pointer to the bottom
        // According to the register specs, x2 register (2) is the stack pointer
        unsafe { (*ret_proc.frame).regs[2] = STACK_ADDR + (STACK_PAGES * PAGE_SIZE); }
        // Map stack on the MMU
        let pt;
        unsafe {
            pt = &mut *ret_proc.root;
        }
        let saddr = ret_proc.stack as usize;
        // Map stack onto the user process' virtual memory
        for i in 0..STACK_PAGES {
            let addr = i * PAGE_SIZE;
            map(pt, STACK_ADDR + addr, saddr + addr, EntryBits::UserReadWrite.val(), 0);
            println!("Set stack from 0x{:016x} -> 0x{:016x}", STACK_ADDR + addr, saddr + addr);
        }

        // Map function pointer to it's own virtual address on the MMU
        for i in 0..=100 {
            let modifier = i * 0x1000;
            map(pt, func_vaddr + modifier, func_addr + modifier, EntryBits::UserReadWriteExecute.val(), 0);
        }
        
        // Map the make_syscall function on the MMU
        map(pt, 0x8000_0000, 0x8000_0000, EntryBits::UserReadExecute.val(), 0);
        // Return the newly created process structure
        ret_proc
    }
}

// When the process structure is dropped, we need to deallocate the memory allocated to it as well
impl Drop for Process {
    fn drop(&mut self) {
        // Deallocate stack pages
        dealloc(self.stack);
        unsafe {
            // Unmap deallocate all page tables except root page table
            unmap(&mut *self.root);
        }
        dealloc(self.root as *mut u8);
    }
}

// The private data in a process contains information
// that is relevant to where we are, including the path
// and open file descriptors.
pub struct ProcessData {
	cwd_path: [u8; 128],
}

// This is private data that we can query with system calls.
// If we want to implement CFQ (completely fair queuing), which
// is a per-process block queuing algorithm, we can put that here.
impl ProcessData {
	pub fn zero() -> Self {
		ProcessData { cwd_path: [0; 128], }
	}
}