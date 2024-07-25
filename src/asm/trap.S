# Assembly-level trap handler

.global asm_trap_vector

asm_trap_vector:
    csrr a0, mtval
    wfi
    j   asm_trap_vector