// Platform level interrupt controller
// PLIC is MMIO, so we read and write to specific memory locations to address registers
const PLIC_PRIORITY: usize = 0x0c00_0000;
const PLIC_INT_ENABLE: usize = 0x0c00_2000;
const PLIC_THRESHOLD: usize = 0x0c20_0000;
const PLIC_CLAIM: usize = 0x0c20_0004;

// Enable an interrupt id
pub fn enable(id: u32) {
    let enables = PLIC_INT_ENABLE as *mut u32;
    // The PLIC_INT_ENABLE is a 32 bit register, with each bit specifying the index of an interrupt
    // To enable interrupts from that particular source, we set the bit id in the PLIC_INT_ENABLE
    let bit_id = 1 << id;
    unsafe {
        // Get value of PLIC_INT_ENABLE and add the interrupt to enable
        enables.write_volatile(enables.read_volatile() | bit_id);
    }
}

// Set interrupt priority for id
// Priorities can be in the range 0..7
// Therefore get the the last 3 bits as the actual priority
pub fn set_priority(id: u32, prio: u8) {
    let cutoff_prio = prio as u32 & 7;
    let priority = PLIC_PRIORITY as *mut u32;
    unsafe {
        // Location of each interrupt's priority is given by:
        // PLIC_PRIORITY + id * 4
        // The add will move by 4 bytes, since we use a 32 bit pointer
        priority.add(id as usize).write_volatile(cutoff_prio);
    }
}

// Set global threshold for all interrupts
// Interrupt with priorities below this threshold will be disabled
// Threshold can be in the range 0..7
pub fn set_threshold(threshold: u8) {
    let cutoff_threshold = threshold as u32 & 7;
    let threshold_ptr = PLIC_THRESHOLD as *mut u32;
    unsafe {
        threshold_ptr.write_volatile(cutoff_threshold);
    }
}

// Get next available interrupt through the claim register
// The PLIC gives the id of the next interrupting device sorted by priority
pub fn next() -> Option<u32> {
    let claim_ptr = PLIC_CLAIM as *const u32;
    let claim_id;
    unsafe {
        claim_id = claim_ptr.read_volatile();
    }

    if claim_id == 0 {
        None
    } else {
        Some(claim_id)
    }
}

// Complete the interrupt
pub fn complete(id: u32) {
    let complete_ptr = PLIC_CLAIM as *mut u32;
    unsafe {
        complete_ptr.write_volatile(id);
    }
}