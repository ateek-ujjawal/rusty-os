use core::fmt::{Write, Result};

pub struct Uart {
    base_addr: usize
}

// Implement write trait for Uart to use the write! macro with it
impl Write for Uart {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.bytes() {
            self.put(c.clone());
        }

        Ok(())
    }
}

impl Uart {
    pub fn new(base_addr: usize) -> Self {
        Uart {
            base_addr
        }
    }

    pub fn init(&mut self) {
        let ptr = self.base_addr as *mut u8;
        unsafe {
            // Set the 0th and 1st bit of LCR to 1 respectively
            // LCR of the UART chip is at base_addr + 3 offset
            // This will set the word length to be 8 bits
            let lcr = (1 << 0) | (1 << 1);
            ptr.add(3).write_volatile(lcr);
    
            // Set 0th bit of FIFO register to 1
            // FIFO control register is at base_addr + 2 offset
            // This enables FIFO reads and writes of data to UART
            let fcr = 1 << 0;
            ptr.add(2).write_volatile(fcr);
    
            // Enable receiver buffer interrupts by setting 0th bit to 1
            // IER is at base_addr + 1
            // Raises CPU interrupt whenever data is added to the receiver
            let ier = 1 << 0;
            ptr.add(1).write_volatile(ier);
    
            // Calculate divisor to set the signaling rate(in baud)
            // For QEMU, we do not need to calculate the divisor,
            // but on real hardware it would be calculated as follows
            // Based on UART NS16550A chipset spec
            // divisor = ceil( (clock_hz) / baud_sps * 16)
            // For a global clock rate of 22.729 MHz to a signaling rate of 2400 baud.
            // divisor = ceil( 22_729_000 / 2400 * 16)
            // divisor = ceil( 22_729_000 / 38_400)
            // divisor = ceil( 591.01 ) = 592
    
            // Split divisor into two parts of 8 bits
            // giving divisor's most and least bits
            let divisor: u16 = 592;
            let divisor_least: u8 = (divisor & 0xff).try_into().unwrap();
            let divisor_most: u8 = (divisor >> 8).try_into().unwrap();
    
            // Set divisor latch access bit to 1 in LCR, which is the 7th bit
            // This will signify base_addr + 0 and base_addr + 1 as DLL and DLM
            // instead of transmitting and receiving registers
            ptr.add(3).write_volatile(lcr | (1 << 7));
    
            // Now write the divisor most into DLM and divisor least into DLL
            // DLL is now at THR/RBR(base_addr + 0), DLM is at IER(base_addr + 1)
            ptr.add(0).write_volatile(divisor_least);
            ptr.add(1).write_volatile(divisor_most);
    
            // Now that the divisor has been set, we can close the latch
            // By setting the divisor latch access bit to 0
            ptr.add(3).write_volatile(lcr);
        }
    }
    
    pub fn get(&mut self) -> Option<u8> {
        let ptr = self.base_addr as *mut u8;
    
        unsafe {
            // Read the 0th bit from Line status register
            // Which tells us if data is ready to be read or not
            if ptr.add(5).read_volatile() & 1 == 0 {
                // DR bit is not set, therefore return None
                None
            } else {
                // DR bit is set, data is ready
                Some(ptr.add(0).read_volatile())
            }
        }
    }
    
    pub fn put(&mut self, c: u8) {
        let ptr = self.base_addr as *mut u8;
    
        unsafe {
            // Ready to transmit/write to UART
            ptr.add(0).write_volatile(c);
        }
    }
}