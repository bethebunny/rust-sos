use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

use spin::Mutex;

use super::page_table;
use super::PAGE_SIZE;

const KERNEL_HEAP_START: u64 = 0x4444_4444_0000;
const KERNEL_HEAP_SIZE: u64 = 100 * 1024;

// Safety: This function maps pages to frames yielded by next_frame.
// It is only safe as long as every frame yielded is never mapped elsewhere.
pub unsafe fn init_kernel_heap(next_frame: &mut dyn FnMut() -> u64) {
    init_kernel_heap_unsafe(next_frame);
}

// "safe" private function to force marking unsafe behavior
fn init_kernel_heap_unsafe(next_frame: &mut dyn FnMut() -> u64) {
    // TODO: kernel logs
    crate::println!("Initializing kernel heap");
    let kernel_heap_pages =
        (KERNEL_HEAP_START..KERNEL_HEAP_START + KERNEL_HEAP_SIZE).step_by(PAGE_SIZE);
    let page_table = unsafe { page_table::l4::PageTable::get() };
    for page in kernel_heap_pages {
        match unsafe { page_table.map_if_unmapped(page, next_frame) } {
            Ok(()) => (),
            Err(err) => panic!("Failed to map kernel heap: {:#?}", err),
        }
    }
}

pub struct KernelHeapAllocator;

pub struct BumpAllocator {
    heap_start: u64,
    heap_size: u64,
    next: u64,
    allocations: usize,
}

impl BumpAllocator {
    pub const unsafe fn new() -> Self {
        BumpAllocator {
            heap_start: KERNEL_HEAP_START,
            heap_size: KERNEL_HEAP_SIZE,
            next: KERNEL_HEAP_START,
            allocations: 0,
        }
    }

    pub fn upper_bound(&self) -> u64 {
        self.heap_start + self.heap_size
    }
}

// Assumes that align is a power of 2
fn align_up(address: u64, align: u64) -> u64 {
    (address + align - 1) & !(align - 1)
}

pub struct Locked<T> {
    value: Mutex<T>,
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.value.lock();
        let requested = align_up(allocator.next, layout.align() as u64);
        let next = requested + layout.size() as u64;
        if next >= allocator.upper_bound() {
            null_mut()
        } else {
            allocator.next = next;
            allocator.allocations += 1;
            next as *const u8 as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut allocator = self.value.lock();
        allocator.allocations -= 1;
        if allocator.allocations == 0 {
            allocator.next = allocator.heap_start;
        }
    }
}

pub type Allocator = Locked<BumpAllocator>;

impl Allocator {
    pub const unsafe fn new() -> Self {
        Locked {
            value: Mutex::new(BumpAllocator::new()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::boxed::Box;
    #[test_case]
    fn test_box() {
        for i in 0..KERNEL_HEAP_SIZE {
            let v = Box::new(i);
            assert_eq!(i, *v);
        }
    }
}
