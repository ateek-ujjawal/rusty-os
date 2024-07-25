# bootloader for the os

.option norvc
.section .data

.section .text.init
.global _start

_start:
    # Read mhartid csr into t0 register
    csrr t0, mhartid
    # Check if t0 is not zero and branch to a loop (f means forward reference)
    bnez t0, 3f
    # Write satp csr with zero value
    csrw satp, zero

# Disallow assembler from assuming gp is initialized, takes value from pc
.option push
.option norelax
    la  gp, _global_pointer
.option pop

# Make memory of BSS section as zero
    la  a0, _bss_start
    la  a1, _bss_end
    bgeu a0, a1, 2f

1:
    sd  zero, (a0)
    addi a0, a0, 8
    bltu a0, a1, 1b
2:
    # This is for setting up CPU machine mode
    # Control registers, set stack pointer, mstatus, mepc, mtvec, mie registers
    # and return to main function

    # Stack grows from bottom to top, so start from _stack_end
    la  sp, _stack_end

    # Set bits 11 and 12 to 11(machine mode)
    li  t0, 0b11 << 11
    csrw mstatus, t0

    # Machine exception program counter, address to return to when returning from a trap, here it is kinit(defined by Rust)
    la  t1, kinit
    csrw mepc, t1

    # Do not allow interrupts while running kinit
    csrw mie, zero

    # Set return address to 2f after Rust's kinit function is over
    la  ra, 2f
    mret

2:
    # Rust will return here after kinit
    # Set supervisor mode in mstatus(bits 11 and 12 = 01) and enable supervisor(bit 5) and machine(bit 7) interrupts
    li  t0, (0b01 << 11) | (1 << 7) | (1 << 5)
	csrw mstatus, t0

    # Set supervisor trap vector to asm_trap_vector
    la  t2, asm_trap_vector
	csrw stvec, t2

    # Set kmain as entry after mret
	la  t1, kmain
	csrw mepc, t1

	# Setting mie (machine interrupt enable) register:
	# This register takes the same bits as mideleg
	# 1 << 1    : Supervisor software interrupt enable (SSIE=1 [Enabled])
	# 1 << 5    : Supervisor timer interrupt enable (STIE=1 [Enabled])
	# 1 << 9    : Supervisor external interrupt enable (SEIE=1 [Enabled])
    li  t2, (1 << 1) | (1 << 5) | (1 << 9)
	csrw mie, t2

	mret

3:

# Loop(park the core/shut it down)
4:
    wfi
    # here b means a backward reference
    j 4b