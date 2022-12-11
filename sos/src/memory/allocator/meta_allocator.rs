use core::{alloc::Allocator, ptr::NonNull};

use hashbrown::HashMap;

use crate::collections::hash_map::SimpleBuildHasher;

use super::{
    big_region_allocator::BigRegionAllocator,
    bootstrap_allocator::{Locked, MutAllocator},
};

// Maybe SystemAllocator is a better name?
// - Should probably own a ResourceAllocator
// - Needs to make system-wide assumptions about memory layouts
// - Probably unreasonable to allow more than one?
pub struct MetaAllocator<A: Allocator + Clone> {
    // fixed_size_allocators: [dyn FixedSizeAllocator; _];
    fixed_size_lookup_table: [NonNull<dyn Allocator>; 512],
    big_region_allocator: Locked<BigRegionAllocator>,
    // TODO: allocator for blocks between say 512 and 4096 bytes

    // TODO: figure this out
    // - maybe need access to ResourceAllocator
    // - we want to be able to proactively tell this allocator to grab more memory before it's full
    // - else risk deadlock where the ResourceAllocator's underlying allocator runs out of memory
    //     during another alloc
    // backup_allocator: A

    // pointers to allocators for a given memory region
    // - keys are vmem pointers >> 20, in other words 1 pointer per l1 page (2MB of vmem)
    // - values are pointers to the unique allocator responsible for that vmem range
    // - on deallocate, we use this hash to determine the correct allocator to route to
    responsible_allocators: HashMap<usize, NonNull<dyn Allocator>, SimpleBuildHasher, A>,
}

unsafe impl<A: Allocator + Clone> MutAllocator for MetaAllocator<A> {
    fn allocate(
        &mut self,
        layout: core::alloc::Layout,
    ) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
        let allocator = if layout.size() < self.fixed_size_lookup_table.len() {
            let ptr = self.fixed_size_lookup_table[layout.size()];
            unsafe { ptr.as_ref() }
        } else {
            &self.big_region_allocator
        };
        allocator.allocate(layout)
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: core::alloc::Layout) {
        // TODO: >>20 should probably be encoded / abstracted a _little_ bit
        // TODO: probably if it's not known assume it's the big_region_allocator
        //       rather than trying to store hash lookups for all of those?
        let responsible_allocator = self.responsible_allocators[&(ptr.addr().get() >> 20)].as_ref();
        responsible_allocator.deallocate(ptr, layout);
    }
}
