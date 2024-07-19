#![no_main]
#![no_std]

use core::{panic::PanicInfo, arch::asm, arch::global_asm};

global_asm!(include_str!("asm/boot.S"));
global_asm!(include_str!("asm/trap.S"));

#[macro_export]
macro_rules! print
{
	($($args:tt)+) => ({

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
	if let Some(_p) = info.location() {
		println!(
					"line {}, file {}: {}",
					p.line(),
					p.file(),
					info.message().unwrap()
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
}

