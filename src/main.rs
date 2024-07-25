#![no_main]
#![no_std]
#![feature(allocator_api,
           alloc_error_handler)]

extern crate alloc;
use alloc::{boxed::Box, string::String, vec};
use core::{arch::{asm, global_asm}, panic::PanicInfo};

global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));
global_asm!(include_str!("asm/mem.S"));

// Since the default print! macro prints to stdout, we need to make our own
// To print write to the UART
#[macro_export]
macro_rules! print
{
	// Tells rust to match the pattern given to print
	// Use '$' as a meta-variable, use $args:tt to tell rust it is a token-tree argument
	// Use '+' to tell there may be one or more match here, to compile need at least one argument
	// Use => to tell rust what to run when a match is found
	($($args:tt)+) => ({
		use core::fmt::Write;
		let _ = write!(crate::uart::Uart::new(0x1000_0000), $($args)+);
	});
}

#[macro_export]
macro_rules! println
{
	() => ({
		print!("\r\n")
	});
	($fmt:expr) => ({
		print!(concat!($fmt, "\r\n"))
	});
	($fmt:expr, $($args:tt)+) => ({
		print!(concat!($fmt, "\r\n"), $($args)+)
	});
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("Aborting: ");
	if let Some(p) = info.location() {
		println!(
					"line {}, file {}: {}",
					p.line(),
					p.file(),
					info.message()
		);
	}
	else {
		println!("no information available.");
	}

    abort();
}

// Wait for interrupts(sleep cores), when calling panic handler
#[no_mangle]
extern "C"
fn abort() -> ! {
	loop {
		unsafe {
			asm!("wfi");
		}
	}
}	

// CONSTANTS

// Imported as symbols from asm/mem.S
extern "C" {
	static TEXT_START: usize;
	static TEXT_END: usize;
	static DATA_START: usize;
	static DATA_END: usize;
	static RODATA_START: usize;
	static RODATA_END: usize;
	static BSS_START: usize;
	static BSS_END: usize;
	static KERNEL_STACK_START: usize;
	static KERNEL_STACK_END: usize;
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
	static mut KERNEL_TABLE: usize;
}

// Identity map a range of addresses using MMU
pub fn id_map_range(root: &mut page::Table, start: usize, end: usize, bits: i64) {
	let mut memaddr = start & !(page::PAGE_SIZE - 1); // Align starting address at 4kb page boundary
	let num_kb_pages = (page::align_val(end, 12) - start) / page::PAGE_SIZE; // Get number of pages to allocate from memaddr

	// Map 4kb pages starting from memaddr with amount as number_kb_pages
	for _ in 0..num_kb_pages {
		page::map(root, memaddr, memaddr, bits, 0);
		memaddr += 1 << 12;
	}
}

#[no_mangle]
// Kinit executes in mode 3 which is machine mode(MPP = 11)
// The job of kinit is to setup the MMU and enter supervisor mode
// This will layout the memory according to virtual addresses
extern "C" fn kinit() {
	// Init uart for debugging purposes
	uart::Uart::new(0x1000_0000).init();
	// Init paged memory and kernel memory
	page::init();
	kmem::init();

	// Get address of root kernel page table and heap head
	let root_ptr = kmem::get_page_table();
	let root_u = root_ptr as usize;
	// Borrow the root table
	let mut root = unsafe {
		root_ptr.as_mut().unwrap()
	};
	let kheap_head = kmem::get_head() as usize;
	let total_pages = kmem::get_num_allocations();
	println!();
	println!();

	unsafe {
		println!("TEXT:   0x{:x} -> 0x{:x}", TEXT_START, TEXT_END);
		println!("RODATA: 0x{:x} -> 0x{:x}", RODATA_START, RODATA_END);
		println!("DATA:   0x{:x} -> 0x{:x}", DATA_START, DATA_END);
		println!("BSS:    0x{:x} -> 0x{:x}", BSS_START, BSS_END);
		println!("STACK:  0x{:x} -> 0x{:x}", KERNEL_STACK_START, KERNEL_STACK_END);
		println!("HEAP:   0x{:x} -> 0x{:x}", kheap_head, kheap_head + total_pages * 4096);
	}

	// Map the kernel heap virtual address
	id_map_range(&mut root, kheap_head, kheap_head + total_pages * 4096, page::EntryBits::ReadWrite.val());

	unsafe {
		// Map the heap descriptors(for user space memory)
		// and TEXT, RODATA, DATA, BSS, STACK sections
		let num_pages = HEAP_SIZE / page::PAGE_SIZE;
		id_map_range(&mut root, HEAP_START, HEAP_START + num_pages, page::EntryBits::ReadWrite.val());

		id_map_range(&mut root, TEXT_START, TEXT_END, page::EntryBits::ReadExecute.val());

		// Map the ROdata section
		// In the linker rodata is put into the text section
		// however it does not matter as long as it is read only
		id_map_range(&mut root, RODATA_START, RODATA_END, page::EntryBits::ReadExecute.val());

		id_map_range(&mut root, DATA_START, DATA_END, page::EntryBits::ReadWrite.val());

		id_map_range(&mut root, BSS_START, BSS_END, page::EntryBits::ReadWrite.val());

		id_map_range(&mut root, KERNEL_STACK_START, KERNEL_STACK_END, page::EntryBits::ReadWrite.val());
	}

	// Map virtual addresses for the UART, CLINT and PLIC chips
	// UART
	page::map(&mut root, 0x1000_0000, 0x1000_0000, page::EntryBits::ReadWrite.val(), 0);
	// CLINT
	//  -> MSIP
	page::map(&mut root, 0x0200_0000, 0x0200_0000, page::EntryBits::ReadWrite.val(), 0);
	//  -> MTIMECMP
	page::map(&mut root, 0x0200_b000, 0x0200_b000, page::EntryBits::ReadWrite.val(), 0);
	//  -> MTIME
	page::map(&mut root, 0x0200_c000, 0x0200_c000, page::EntryBits::ReadWrite.val(), 0);
	// PLIC
	id_map_range(&mut root, 0x0c00_0000, 0x0c00_2000, page::EntryBits::ReadWrite.val());
	id_map_range(&mut root, 0x0c20_0000, 0x0c20_8000, page::EntryBits::ReadWrite.val());

	page::print_page_allocations();

	// The following code shows how to convert a virtual address to a physical address
	// When user applications see memory they only see virtual addresses, so we have to translate it to a physical address behind the scenes
	let p = 0x8005_7000 as usize;
	let m = page::virt_to_phys(&root, p).unwrap_or(0);
	println!("Walk 0x{:x} = 0x{:x}", p, m);

	unsafe {
		// Store the root kernel page table in a constant, since it will keep changing
		// when switching from supervisor to machine mode or clearing the satp
		KERNEL_TABLE = root_u;
	}

	// Write root page table into the satp register
	// which is basically the mode(8 for Sv39) and the root kernel page table's address
	// we will shift the address of the table by 12 bits to the right to fit in the satp correctly
	// Also write the sfence.vma so the MMU always grabs a fresh copy of the tables instead of loading from cache
	let root_ppn = root_u >> 12;
	let satp_val = 8 << 60 | root_ppn;
	unsafe {
		asm!("csrw satp, {}", in(reg) satp_val);
		asm!("sfence.vma zero, {}", in(reg) 0);
	}
}

// Enter Rust code here(kmain)
#[no_mangle]
extern "C"
fn kmain() {
	// kmain should be reached when supervisor mode is turned on by kinit
	// Trap vector will be setup and MMU will be turned on
	println!("Welcome to Rusty OS!");

	// Get a new pointer to the UART
	let mut my_uart = uart::Uart::new(0x1000_0000);

	// Check if global allocator
	// and virtual memory works as expected
	{	
		let k = Box::<u32>::new(100);
		println!("Boxed value: {}", *k);
		kmem::print_kmem();
		let sparkle_heart = vec![240, 159, 146, 150];
		let sparkle_heart = String::from_utf8(sparkle_heart).unwrap();
		println!("String = {}", sparkle_heart);
	}

	// Test if uart reading works
	// Read user input from UART and write it to UART as well(MMIO UART)
	loop {
		if let Some(c) = my_uart.get() {
			match c {
				8 => {
					// 8 is a backspace, so go back, print a space, then go back again
					print!("{} {}", 8 as char, 8 as char);
				},
				10 | 13 => {
					// Newline/carriage return
					println!();
				},
				_ => {
					// Print everything else normally
					print!("{}", c as char);
				}
			}
		}
	}
}

// OS Modules go here
pub mod uart;
pub mod page;
pub mod kmem;