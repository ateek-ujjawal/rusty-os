// Sub-page level allocation system(byte-level allocator)
// This file is used for allocating kernel memory only, not user space memory!

use core::ptr::null_mut;

use crate::page::{align_val, zalloc, Table, PAGE_SIZE};

#[repr(usize)]
enum AllocListFlags {
    Taken = 1 << 63 // Rest of the bits for the size
}

impl AllocListFlags {
    pub fn val(self) -> usize {
        self as usize
    }
}

// Store the taken flag and remaining memory after this AllocList
struct AllocList {
    pub flags_and_size: usize
}

impl AllocList {
    pub fn is_taken(&self) -> bool {
        self.flags_and_size & AllocListFlags::Taken.val() != 0
    }

    pub fn is_free(&self) -> bool {
        !self.is_taken()
    }

    pub fn set_taken(&mut self) {
        self.flags_and_size |= AllocListFlags::Taken.val();
    }

    pub fn set_free(&mut self) {
        self.flags_and_size &= !AllocListFlags::Taken.val();
    }

    pub fn set_size(&mut self, sz: usize) {
        let k = self.is_taken();
        self.flags_and_size = sz & !AllocListFlags::Taken.val();
        if k {
            self.set_taken();
        }
    }

    pub fn get_size(&self) -> usize {
        self.flags_and_size & !AllocListFlags::Taken.val()
    }
}

// We will start kernel memory allocations from here by searching for free memory
static mut KMEM_HEAD: *mut AllocList = null_mut();
// Keep track of how much memory is allocated currently
static mut KMEM_ALLOC: usize = 0;
// Keep track of where the kernel page table is
static mut KMEM_PAGE_TABLE: *mut Table = null_mut();

// Safe wrapper functions around unsafe operation
pub fn get_head() -> *mut u8 {
    unsafe { KMEM_HEAD as *mut u8 }
}

pub fn get_page_table() -> *mut Table {
    unsafe { KMEM_PAGE_TABLE as *mut Table }
}

pub fn get_num_allocations() -> usize {
    unsafe { KMEM_ALLOC }
}

// Allocate memory for the kernel
// Only need 64 pages for now
pub fn init() {
    unsafe {
        let k_alloc = zalloc(64);
        assert!(!k_alloc.is_null());
        KMEM_ALLOC = 64;
        KMEM_HEAD = k_alloc as *mut AllocList;
        (*KMEM_HEAD).set_free();
        (*KMEM_HEAD).set_size(KMEM_ALLOC * PAGE_SIZE);
        KMEM_PAGE_TABLE = zalloc(1) as *mut Table;
    }
}

// Byte allocation for kernel use
pub fn kmalloc(sz: usize) -> *mut u8 {
    unsafe {
        // Align the size to byte boundary and add size of AllocList to be stored along with it
        let size = align_val(sz, 3) + size_of::<AllocList>();

        // Get the head and tail of the kernel memory
        let mut head = KMEM_HEAD;
        let tail = (head as *mut u8).add((*head).get_size()) as *mut AllocList;

        while head < tail {
            // If free head/chunk is found, allocate it
            if (*head).is_free() && size < (*head).get_size() {
                let chunk_size = (*head).get_size();
                let rem = chunk_size - size;
                (*head).set_taken();
                // If there is space for the AllocList, mark the remaining chunk as free for use
                if rem > size_of::<AllocList>() {
                    let next = (head as *mut u8).add(size) as *mut AllocList;
                    (*next).set_free();
                    (*next).set_size(rem);
                    (*head).set_size(size);
                } else {
                    // Take the entirety of the remaining chunk
                    (*head).set_size(chunk_size);
                }
                // Return the pointer after the alloc list
                return head.add(1) as *mut u8;
            } else {
                // Get the next free chunk after this taken memory
                head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
            }
        }
    }
    // If we reach here, we did not find any free chunk of kernel memory
    null_mut()
}

// Zeroed out kernel memory allocation
pub fn kzmalloc(sz: usize) -> *mut u8 {
    let size = align_val(sz, 3);
    let ret = kmalloc(size);

    if !ret.is_null() {
        for i in 0..size {
            unsafe {
                (*ret.add(i)) = 0;
            }
        }
    }
    ret
}