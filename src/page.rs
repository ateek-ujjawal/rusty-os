use core::ptr::null_mut;

// Wrangle symbols directly from the linker in C format
// Reading these symbols is unsafe!
extern "C" {
	static HEAP_START: usize;
	static HEAP_SIZE: usize;
}

// Mark start of the memory space by ALLOC_START
// PAGE_SIZE is 4KB, Hence PAGE_ORDER is 12
// Left-shifting by 12 gives us 4KB PAGE_SIZE
static mut ALLOC_START: usize = 0;
const PAGE_ORDER: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_ORDER;

// Align val to the order given in the argument
// order means 2^order
// This will align(basically rounding) val to a 2^order boundary
pub const fn align_val(val: usize, order: usize) -> usize {
	let o = (1 << order) - 1;
	(val + o) & !o
}

// Mark enum offsets at 8-bit boundaries
#[repr(u8)]
pub enum PageBits {
	Empty = 0,
	Taken = 1 << 0, // Last bit is 1
	Last = 1 << 1 // 2nd last bit is 1
}

// Get flag as an unsigned 8-bit integer
impl PageBits {
	pub fn val(self) -> u8 {
		self as u8
	}
}

// Page structure(holds flags for each page and NOT the actual page itself!)
pub struct Page {
	flags: u8
}

impl Page {
	pub fn is_taken(&self) -> bool {
		self.flags & PageBits::Taken.val() == 1
	}

	pub fn is_last(&self) -> bool{
		self.flags & PageBits::Last.val() != 0
	}

	pub fn is_free(&self) -> bool {
		!self.is_taken()
	}

	pub fn clear(&mut self) {
		self.flags = PageBits::Empty.val();
	}

	pub fn set_flag(&mut self, flag: PageBits) {
		self.flags |= flag.val();
	}
}

// Init the page structure by clearing all pages
// Specify start of paged memory through ALLOC_START
// No need to clear page memory itself here!
pub fn init() {
	unsafe {
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;

		// Clear all page structures
		for i in 0..num_pages {
			(*ptr.add(i)).clear();
		}

		// Align ALLOC_START after the page structure table
		// to the order of PAGE_SIZE(4096 bytes)
		ALLOC_START = align_val(HEAP_START + num_pages * size_of::<Page>(), PAGE_ORDER);
	}
}

// Find a contiguous allocation of page memory
pub fn alloc(pages: usize) -> *mut u8 {
	assert!(pages > 0);

	unsafe {
		// Calculate total number of pages and pointer to the start of the heap
		let num_pages = HEAP_SIZE / PAGE_SIZE;
		let ptr = HEAP_START as *mut Page;

		// At most, the page index can be num_pages - pages and not anything more
		for i in 0..(num_pages - pages) {
			// Find a free page
			let mut found = false;

			if (*ptr.add(i)).is_free() {
				// Page found which is free
				// Set found as true
				found = true;

				for j in i..(i + pages) {
					if (*ptr.add(j)).is_taken() {
						found = false;
						break;
					}
				}
			}

			// If we reach here, then we have found contiguous pages
			// Now we need to return a pointer to the start of paged memory
			if found {
				// Set taken flag for all pages
				for k in i..(i + pages - 1) {
					(*ptr.add(k)).set_flag(PageBits::Taken);
				}

				// Set taken and last flag for last page
				(*ptr.add(i + pages - 1)).set_flag(PageBits::Taken);
				(*ptr.add(i + pages - 1)).set_flag(PageBits::Last);

				// Return a pointer to the start of the paged memory
				return (ALLOC_START + PAGE_SIZE * i) as *mut u8;
			}
		}
	}

	// If we get here then no contiguous page was found, return null pointer
	null_mut()
}

// Deallocate a page
// Argument gives an absolute page pointer, so need to convert that to a page index
// To manage it's page structure
pub fn dealloc(ptr: *mut u8) {
	// Don't free a null page!
	assert!(!ptr.is_null());

	unsafe {
		// Calculate page index by subtracting ptr from top of useable memory
		// Then add this to the heap_start to calculate page_struct_address offset from HEAP_START
		let page_struct_addr = HEAP_START + ((ptr as usize - ALLOC_START) / PAGE_SIZE);

		// Assert if page_addr calculated is in the usable heap range
		assert!(page_struct_addr >= HEAP_START && page_struct_addr < HEAP_START + HEAP_SIZE);

		let mut p = page_struct_addr as *mut Page;

		// Run loop till last page and if every page is taken
		// Clear the page structures one by one
		while (*p).is_taken() && !(*p).is_last() {
			(*p).clear();
			p = p.add(1);
		}

		// Check if this is not the last page
		// If so, then the heap is messed up
		// Possible double-free since non-taken page encountered before last page
		assert!((*p).is_last() == true, "Possible double-free encountered");

		// If we reach here, then it is safe to clear the last page
		(*p).clear();
	}
}

// Allocates AND zeroes out the pages for kernel/application use
pub fn zalloc(pages: usize) -> *mut u8 {
	// Allocate pages through alloc
	let page_ptr = alloc(pages);
	if !page_ptr.is_null() {
		// Size of page(in 8 byte words)
		let size = (PAGE_SIZE * pages) / 8;
		// Use big_ptr which writes in 8 byte words instead of byte-by-byte
		// This is an optimization over u8 as we need to use lesser instructions to zero out the pages
		// For 1 page, this will use 4096 * 1 / 8 = 512 loops and instructions as opposed to 4096 loops
		// Used sd(store doubleword) instruction instead of sb(store byte)
		let big_ptr = page_ptr as *mut u64;
		for i in 0..size {
			unsafe {
				(*big_ptr.add(i)) = 0;
			}
		}
	}
	page_ptr
}

// Print page allocations
pub fn print_page_allocations() {
    unsafe {
        // Calculate number of pages, start of page structure table in heap, end of the table
        // and start of paged memory and end of paged memory
        let num_pages = HEAP_SIZE / PAGE_SIZE;
        let mut beg = HEAP_START as *mut Page;
        let end = beg.add(num_pages);
        let alloc_beg = ALLOC_START;
        let alloc_end = ALLOC_START + (num_pages * PAGE_SIZE);

        // Print the above values
        println!();
        println!("PAGE ALLOCATION TABLE\nMETA: {:p} -> {:p}\nPHYS: 0x{:x} -> 0x{:x}", beg, end, alloc_beg, alloc_end);
		println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
		let mut num = 0;
        while beg < end {
            if(*beg).is_taken() {
                // If page is taken, print number of pages(and page addresses) allocated till last page
                let start = beg as usize;
                // Calculate starting address of taken pages
                let start_memaddr = alloc_beg + (PAGE_SIZE * (start - HEAP_START));
                print!("0x{:x} => ", start_memaddr);
                loop {
                    num += 1;
                    if (*beg).is_last() {
                        let end = beg as usize;
                        // Calculate ending address of taken pages
                        let end_memaddr = alloc_beg + (PAGE_SIZE * (end - HEAP_START)) + PAGE_SIZE - 1;
                        print!("0x{:x}: {:>3} page(s)", end_memaddr, end - start + 1);

                        // Last page found, break out of loop
                        println!(".");
                        break;
                    }
                    beg = beg.add(1);
                }
            }
            beg = beg.add(1);
        }
        println!("~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~");
        println!("Allocated: {:>6} pages ({:>10} bytes).", num, num * PAGE_SIZE);
        println!("Free: {:>6} pages ({:>10} bytes).", (num_pages - num), (num_pages - num) * PAGE_SIZE);
        println!();
    }
}

// MMU

// Represent the last 8 bits of the page table entry
// Describes various flags
#[repr(i64)]
#[derive(Clone, Copy)]
pub enum EntryBits {
	None = 0,
	Valid = 1 << 0,
	Read = 1 << 1,
	Write = 1 << 2,
	Execute = 1 << 3,
	User = 1 << 4,
	Global = 1 << 5,
	Access = 1 << 6,
	Dirty = 1 << 7
}

impl EntryBits {
	pub fn val(self) -> i64 {
		self as i64
	}
}

// One page table entry has 64 bits
// Leaf PTE means no need to walk through the tables, since physical page(memory) has been found
pub struct Entry {
	pub entry: i64
}

impl Entry {
	pub fn get_entry(&self) -> i64 {
		self.entry
	}

	pub fn set_entry(&mut self, entry: i64) {
		self.entry = entry;
	}

	pub fn is_valid(&self) -> bool {
		self.get_entry() & EntryBits::Valid.val() != 0
	}

	pub fn is_invalid(&self) -> bool {
		!self.is_valid()
	}

	// Check if this is leaf PTE(Read, write or execute bits are 1)
	pub fn is_leaf(&self) -> bool {
		self.get_entry() & 0xe != 0
	}

	pub fn is_branch(&self) -> bool {
		!self.is_leaf()
	}	
}

// Define a page table
// Has 512 entries, each of 8 bytes, giving 512 * 8 = 4096 bytes for one table
pub struct Table {
	pub entries: [Entry; 512]
}

impl Table {
	pub fn len() -> usize {
		512
	}
}

// Maps a virtual address to the given physical address
// root: Root page table
// vaddr: The virtual address as specified in RISC-V privileged isa
// paddr: The physical address as specified in RISC-V privileged isa
// bits: The 8 entry bits to be set in the page table entry
// level: The levels needed to traverse the page tables to locate the physical address
pub fn map(root: &mut Table, vaddr: usize, paddr: usize, bits: i64, level: usize) {
	// The bits to be set, must have either read, write or execute bit set
	// otherwise this will be a faulty page
	assert!(bits & 0xe != 0);

	// Get the virtual page numbers
	let vpn = [
		vaddr >> 12, // VPN[0]
		vaddr >> 21, // VPN[1]
		vaddr >> 30  // VPN[2]
	];

	// Get the physical page numbers from the physical address
	let ppn = [
		paddr >> 12, // PPN[0]
		paddr >> 21, // PPN[1]
		paddr >> 30  // PPN[2]
	];

	// Get root page table entry
	let mut v = &mut root.entries[vpn[2]];

	// Loop through the levels of the page tables
	// Reverse to loop backwards from level 2
	for i in (level..2).rev() {
		if v.is_invalid() {
			// Valid page table entry not found
			// So allocate a physical page and store it in the entry
			let page_addr = zalloc(1);

			// Get the page address, convert it to an i64 number
			// In the Sv39 scheme, physical addresses start at bit 12 (11:0 reserved for the offset)
			// While the same physical address starts at bit 10 in the page table entry (9:0 reserved for various entry bit flags)
			// So we shift the physical address we get from the hardware MMU by 2 bits to ensure correct alignment
			v.set_entry((page_addr as i64 >> 2) | EntryBits::Valid.val());
		}

		// Get the physical address of next page from the entry and shift it left by 2 to fit into the physical address space(56-bits)
		// as defined in the Sv39 scheme
		// We bitwise and with !0x3ff so that the last 10 bits are zeroed out(we don't need them in branches)
		let entry = ((v.get_entry() & !0x3ff) << 2) as *mut Entry;
		v = unsafe { entry.add(vpn[i]).as_mut().unwrap() };
	}

	// We reach here when we find a leaf(v points to a physical page that is not a page table)
	// Map the given physical page in this entry
	let entry = (ppn[2] << 28) as i64 |
					 (ppn[1] << 19) as i64 |
					 (ppn[0] << 10) as i64 |
					  bits |					// set provided bits
					  EntryBits::Valid.val();	// Valid page
	v.set_entry(entry);
}

// Unmap/deallocate page tables for later use
pub fn unmap(root: &mut Table) {
	// Loop 512 page table entries in the root table
	// which will give page tables corresponding to level 2
	for lv2 in 0..Table::len() {
		let ref entry_lv2 = root.entries[lv2];
		// Check if given entry is valid and is a branch
		if entry_lv2.is_valid() && entry_lv2.is_branch() {
			// Get address of level 1 page table
			let memaddr_lv1 = (entry_lv2.get_entry() & !0x3ff) << 2;
			let table_lv1 = unsafe { (memaddr_lv1 as *mut Table).as_mut().unwrap() };
			for lv1 in 0..Table::len() {
				let ref entry_lv1 = table_lv1.entries[lv1];
				if entry_lv1.is_valid() && entry_lv1.is_branch() {
					// Get address of level 0 page table
					let memaddr_lv0 = (entry_lv1.get_entry() & !0x3ff) << 2;

					// Free the memory address(page), since branches won't exist at level 0
					dealloc(memaddr_lv0 as *mut u8);
				}
			}
			// Free the level 1 page table after freeing the tables inside it
			dealloc(memaddr_lv1 as *mut u8);
		}
	}
}