#![no_main]
#![no_std]

use core::{arch::{asm, global_asm}, panic::PanicInfo, ptr::null_mut};

global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));

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

// Enter Rust code here(kmain)
#[no_mangle]
extern "C"
fn kmain() {
	// Main should initialize all sub-systems and get
	// ready to start scheduling. The last thing this
	// should do is start the timer.

	// Print to UART
	let mut my_uart = uart::Uart::new(0x1000_0000);
	my_uart.init();

	println!("Write to OS succeeded!");
	println!("Now we can write something");

	// Test if uart reading works
	// Read user input from UART and write it to UART as well
	loop {
		if let Some(c) = my_uart.get() {
			match c {
				8 => {
					// 8 is a backspace, so go back, print a space, then go back again
					print!("{}{}{}", 8 as char, ' ', 8 as char);
				},
				10 | 13 => {
					// Newline/carriage return
					println!();
				},
				// An ANSI escape sequence
				// Starts with byte 0x1b
				0x1b => {
					// The next byte should be 0x5b(91)
					// Specifies a left bracket('[')
					if let Some(next_byte) = my_uart.get() {
						if next_byte == 91 {
							// This is a right bracket! We're on our way!
							// The next bytes are parameters to the sequence
							if let Some(b) = my_uart.get() {
								match b as char {
									'A' => {
										println!("That's the up arrow!");
									},
									'B' => {
										println!("That's the down arrow!");
									},
									'C' => {
										println!("That's the right arrow!");
									},
									'D' => {
										println!("That's the left arrow!");
									},
									_ => {
										println!("That's something else.....");
									}
								}
							}
						}
					}
				},
				_ => {
					// Print everything else normally
					print!("{}", c as char);
				}
			}
		}
	}
}

// Wrangle symbols directly from the linker in C format
// Reading these symbols is unsafe!
extern "C" {
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
}

// Mark start of the memory space by ALLOC_START
// PAGE_SIZE is 4KB, Hence PAGE_ORDER is 12
// Left-shifting by 12 gives us 4KB PAGE_SIZE
static mut ALLOC_START: usize = 0;
const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = (1 << PAGE_ORDER);

// Align val to the order given in the argument
// order means 2^order
// This will align(basically rounding) val to a 2^order boundary
pub const fn align_val(val: usize, order: usize) -> usize {
	let o = (1 << order) - 1;
	(val + o) & !o
}

// Mark enum offsets at 8-bit boundaries
#[repr(u8)]
pub enum PageBits {
	Empty = 0,
	Taken = 1,
	Last = 2
}

// Get flag as an unsigned 8-bit integer
impl PageBits {
	pub fn val(self) -> u8 {
		self as u8
	}
}

// Page structure(holds flags for each page and NOT the actual page itself!)
pub struct Page {
	flags: u8
}

impl Page {
	pub fn is_taken(&self) -> bool {
		self.flags & PageBits::Taken.val() == 1
	}

	pub fn is_last(&self) -> bool{
		self.flags & PageBits::Last.val() != 0
	}

	pub fn is_free(&self) -> bool {
		!self.is_taken()
	}

	pub fn clear(&mut self) {
		self.flags = PageBits::Empty.val();
	}

	pub fn set_flag(&mut self, flag: PageBits) {
		self.flags |= flag.val();
	}
}

// Init the page structure by clearing all pages
// Specify start of paged memory through ALLOC_START
// No need to clear page memory itself here!
pub fn init() {
	unsafe {
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;

		// Clear all page structures
		for i in 0..num_pages {
			(*ptr.add(i)).clear();
		}

		// Align ALLOC_START after the page structure table
		// to the order of PAGE_SIZE(4096 bytes)
		ALLOC_START = align_val(HEAP_START + num_pages * size_of::<Page>(), PAGE_SIZE);
	}
}

// Find a contiguous allocation of page memory
pub fn alloc(pages: usize) -> *mut u8 {
	assert!(pages > 0);

	unsafe {
		// Calculate total number of pages and pointer to the start of the heap
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;

		// At most, the page index can be num_pages - pages and not anything more
		for i in 0..(num_pages - pages) {
			// Find a free page
			let mut found = false;

			if (*ptr.add(i)).is_free() {
				// Page found which is free
				// Set found as true
				found = true;

				for j in i..(i + pages) {
					if (*ptr.add(j)).is_taken() {
						found = false;
						break;
					}
				}
			}

			// If we reach here, then we have found contiguous pages
			// Now we need to return a pointer to the start of paged memory
			if found {
				// Set taken flag for all pages
				for k in i..(i + pages - 1) {
					(*ptr.add(k)).set_flag(PageBits::Taken);
				}

				// Set taken and last flag for last page
				(*ptr.add(i + pages - 1)).set_flag(PageBits::Taken);
				(*ptr.add(i + pages - 1)).set_flag(PageBits::Last);

				// Return a pointer to the start of the paged memory
				return (ALLOC_START + PAGE_SIZE * i) as *mut u8;
			}
		}
	}

	// If we get here then no contiguous page was found, return null pointer
	null_mut()
}

// Deallocate a page
// Argument gives an absolute page pointer, so need to convert that to a page index
// To manage it's page structure
pub fn dealloc(ptr: *mut u8) {
	// Don't free a null page!
	assert!(!ptr.is_null());

	unsafe {
		// Calculate page index by subtracting ptr from top of useable memory
		// Then add this to the heap_start to calculate page_struct_address offset from HEAP_START
		let page_struct_addr = HEAP_START + ((ptr as usize - ALLOC_START) / PAGE_SIZE);

		// Assert if page_addr calculated is in the usable heap range
		assert!(page_struct_addr >= HEAP_START && page_struct_addr < HEAP_START + HEAP_SIZE);

		let mut p = page_struct_addr as *mut Page;

		// Run loop till last page and if every page is taken
		// Clear the page structures one by one
		while (*p).is_taken() && !(*p).is_last() {
			(*p).clear();
			p = p.add(1);
		}

		// Check if this is not the last page
		// If so, then the heap is messed up
		// Possible double-free since non-taken page encountered before last page
		assert!((*p).is_last() == true, "Possible double-free encountered");

		// If we reach here, then it is safe to clear the last page
		(*p).clear();
	}
}

// Allocates AND zeroes out the pages for kernel/application use
pub fn zalloc(pages: usize) -> *mut u8 {
	// Allocate pages through alloc
	let page_ptr = alloc(pages);
	if !page_ptr.is_null() {
		// Size of page(in 8 byte words)
		let size = (PAGE_SIZE * pages) / 8;
		// Use big_ptr which writes in 8 byte words instead of byte-by-byte
		// This is an optimization over u8 as we need to use lesser instructions to zero out the pages
		// For 1 page, this will use 4096 * 1 / 8 = 512 loops and instructions as opposed to 4096 loops
		let big_ptr = page_ptr as *mut u64;
		for i in 0..size {
			unsafe {
				(*big_ptr.add(i)) = 0;
			}
		}
	}
	page_ptr
}

// OS Modules go here
pub mod uart;