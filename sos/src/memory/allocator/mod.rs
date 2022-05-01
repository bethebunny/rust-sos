pub mod bootstrap_allocator;
pub mod bump_allocator;
pub mod page_allocator;
pub mod resource_allocator;

use bump_allocator::SyncBumpAllocator;

use super::page_table;
use super::PAGE_SIZE;

const KERNEL_HEAP_START: usize = 0x4444_4444_0000;
const KERNEL_HEAP_SIZE: usize = 100 * 1024;

#[global_allocator]
static ALLOCATOR: SyncBumpAllocator =
    unsafe { SyncBumpAllocator::new(KERNEL_HEAP_START, KERNEL_HEAP_SIZE) };

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

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
        match unsafe { page_table.map_if_unmapped(page as u64, next_frame) } {
            Ok(()) => (),
            Err(err) => panic!("Failed to map kernel heap: {:#?}", err),
        }
    }
}
