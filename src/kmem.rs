// Sub-page level allocation system(byte-level allocator)
// This file is used for allocating kernel memory only, not user space memory!

use core::{alloc::{Layout, GlobalAlloc}, ptr::null_mut};

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
        let tail = (head as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;

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

// Coalesce small freed memory chunks into bigger chunks to reduce fragmentation
pub fn coalesce() {
    unsafe {
        let mut head = KMEM_HEAD;
        let tail = (head as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;

        while head < tail {
            let next = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
            if (*head).get_size() == 0 {
                // Error, size can never be zero, heap must be messed up
                // Break out of the loop
                break;
            } else if next >= tail {
                // We might have moved past the tail
                // In this case size is wrong
                // Break out of the loop
                break;
            } else if (*head).is_free() && (*next).is_free() {
                // Found adjacent free blocks of memory
                // Coalesce them into one
                (*head).set_size((*head).get_size() + (*next).get_size());
            }
            // Check for other free blocks by moving the head
            head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
        }
    }
}

// Free the memory block pointed by this ptr
pub fn kfree(ptr: *mut u8) {
    unsafe {
        if !ptr.is_null() {
            let p = (ptr as *mut AllocList).offset(-1);
            if (*p).is_taken() {
                (*p).set_free();
            }

            // After freeing the AllocList, check for adjacent free blocks
            // and coalesce the memory
            coalesce();
        }
    }
}

// Print the kernel memory space
pub fn print_kmem() {
    unsafe {
        let mut head = KMEM_HEAD;
        let tail = (head as *mut u8).add(KMEM_ALLOC * PAGE_SIZE) as *mut AllocList;
        while head < tail {
            println!("{:p}: Length = {:>10} Taken = {}", head, (*head).get_size(), (*head).is_taken());
            head = (head as *mut u8).add((*head).get_size()) as *mut AllocList;
        }
    }
}

// Define global allocator functions
// A global allocator allows to allocate memory for core data structures
// such as a linked list.
// Since we use our own allocator, we implement the global allocator functions

struct OsGlobalAllocator;

unsafe impl GlobalAlloc for OsGlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        kzmalloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        kfree(ptr)
    }
}

#[global_allocator]
static GA: OsGlobalAllocator = OsGlobalAllocator {};

#[alloc_error_handler]
pub fn alloc_error(l: Layout) -> ! {
    panic!("Allocator failed to allocate {} bytes with {}-byte alignment!", l.size(), l.align());
}