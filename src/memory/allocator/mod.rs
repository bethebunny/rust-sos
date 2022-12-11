pub mod big_region_allocator;
pub mod bootstrap_allocator;
pub mod bump_allocator;
pub mod fixed_size_allocator;
pub mod meta_allocator;
pub mod page_allocator;
pub mod resource_allocator;

use bump_allocator::BumpAllocator;

use self::bootstrap_allocator::Locked;

use super::page_table;
use super::PAGE_SIZE;

const KERNEL_HEAP_START: usize = 0x4444_4444_0000;
const KERNEL_HEAP_SIZE: usize = 100 * 1024;

// TODO:
// - MetaAllocator
//   - How to know which allocator owns which pointers?
//     - Manage allocators in a way such that
//       - No more than 1 allocator per 2MB block (l1 page table)
//       - For each managed 2MB block, put ptr >> 20 in a hash
//       - on deallocate, look up ptr >> 20 to get managing allocator
// - Handoff from bootstrap allocator to MetaAllocator
//   - "Add" bootstrap allocator to MetaAllocator so it can deallocate from it
//   - How to deal with deadlocks where the PageAllocator requires new memory to be mapped?
//     - Backup allocator
//     - Can be simple / slower, rare case to run
//     - Can't rely on thread safety of PageAllocator
//     - Proactively allocates new memory before it is full to avoid deadlocks
// - FixedSizeAllocator
// - Remove `allocator` from module names

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = {
    let alloc = unsafe { BumpAllocator::new(KERNEL_HEAP_START, KERNEL_HEAP_SIZE) };
    Locked::new(alloc)
};

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

// Safety: This function maps pages to frames yielded by next_frame.
// It is only safe as long as every frame yielded is never mapped elsewhere.
pub unsafe fn init_kernel_heap(next_frame: &mut dyn FnMut() -> usize) {
    init_kernel_heap_unsafe(next_frame);
}

// "safe" private function to force marking unsafe behavior
fn init_kernel_heap_unsafe(next_frame: &mut dyn FnMut() -> usize) {
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
