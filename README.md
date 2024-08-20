rusty-os
=
A RISC-V based small operating system written in Rust based on Stephen Marz's blog series.

Features implemented
-
- Bootloader in RISC-V assembly
- No standard library functions
- UART, PLIC chip interrupts
- Virtual Memory and page allocators using MMU in hardware(QEMU)
- Trap handler and system calls
- Process structures
