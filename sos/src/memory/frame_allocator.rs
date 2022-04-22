use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

use super::PAGE_SIZE;

// I need to think about this more carefully. We really want the page allocator to have access
// to some unsize data structures to be able to manage and reclaim pages, and eg. eventually try
// to allocate pages to processes with some sort of affinity characteristics. However since it's
// required for creating the heap for the heap allocator, we can't rely on the heap allocator yet.
//
// What if instead we bootstrap the kernel's heap with manually mapped pages, and then initialize
// the heap allocator, and finally set up the page allocator?
struct FrameAllocator {
    memory_map: &'static MemoryMap,
    iter: &'static mut dyn Iterator<Item = u64>,
}

pub(in crate::memory) fn usable_frames(
    memory_map: &'static MemoryMap,
) -> impl Iterator<Item = u64> {
    memory_map
        .iter()
        .filter(|r| r.region_type == MemoryRegionType::Usable)
        .flat_map(|r| (r.range.start_frame_number..r.range.end_frame_number))
        .map(|frame_number| frame_number * PAGE_SIZE as u64)
}

// impl FrameAllocator {
//     fn new(memory_map: &'static MemoryMap) -> Self {
//         FrameAllocator {
//             memory_map,
//             iter: usable_frames(memory_map),
//         }
//     }
//     fn allocate_page(&mut self) -> u64 {}
//     fn deallocate_page(&mut self, page: u64) {}
// }
