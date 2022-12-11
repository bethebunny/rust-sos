use core::alloc::{AllocError, Layout};
use core::ptr::NonNull;

use crate::memory::PAGE_ALLOCATOR;

use super::bootstrap_allocator::MutAllocator;

// TODO: this should probably not reference the global PAGE_ALLOCATOR; I think it's not
// even a good idea for a global PAGE_ALLOCATOR to be exposed :/
pub struct BigRegionAllocator;

unsafe impl MutAllocator for BigRegionAllocator {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // TODO:
        //  - test allocations for intra-page-size values
        //  - lazily mapped pages for larger allocs
        //  - fancier growable allocation for larger allocs
        PAGE_ALLOCATOR
            .lock()
            .allocate(layout.size())
            .or(Err(AllocError))
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        PAGE_ALLOCATOR
            .lock()
            .deallocate(ptr.as_ptr(), layout.size())
    }
}
