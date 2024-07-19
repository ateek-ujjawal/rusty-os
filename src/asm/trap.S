# Assembly-level trap handler

.section .text
.global asm_trap_vector

asm_trap_vector:
    mret