# bootloader for the os

.option norvc
.section .data
.equ  CONSTANT, 0xdeadbead
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

# Control registers, set stack pointer, mstatus, mepc, mtvec, mie registers
# and return to main function

# Stack grows from bottom to top, so start from _stack_end
la  sp, _stack_end

li a6, CONSTANT
# Set bits 11 and 12 to 11(machine mode), set bits 7(MPIE: Machine previous interrupt enable) to 1 and 3(MIE: Machine's interrupt enable) to 1 to enable interrupts
li  t0, (0b11) << 11 | (1 << 7) | (1 << 3)
csrw mstatus, t0

# Machine exception program counter, address to return to when returning from a trap, here it is kmain(defined by Rust)
la  t1, kmain
csrw mepc, t1

# machine trap vector, kernel function to be called whenever there is a trap
la  t2, asm_trap_vector
csrw mtvec, t2

# enable some interrupts through the machine interrupt enable register
# 1 << 3: Machine's M-mode software interrupt-enable bit
# 1 << 7: Machine's M-mode timer interrupt-enable bit
# 1 << 11: Machine's external interrupt-enable bit

li  t3, (1 << 3) | (1 << 7) | (1 << 11)
csrw mie, t3

# Set return address to park after Rust's main function is over
la  ra, 4f
mret

3:

# Loop(park the core/shut it down)
4:
    wfi
    # here b means a backward reference
    j 4b