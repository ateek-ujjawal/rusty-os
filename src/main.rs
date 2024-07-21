#![no_main]
#![no_std]

use core::{arch::{asm, global_asm}, panic::PanicInfo};

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

	page::init();
	let page_ptr = page::alloc(32);
	page::print_page_allocations();

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

// OS Modules go here
pub mod uart;
pub mod page;