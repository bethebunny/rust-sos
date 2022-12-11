use bitflags::bitflags;
use bootloader::BootInfo;
use lazy_static::lazy_static;
use spin::Mutex;

pub mod allocator;
pub mod frame_allocator;
pub mod page_table;

use allocator::page_allocator::PageAllocator;
use page_table::Err;

const PAGE_SIZE: usize = 4096;

lazy_static! {
    static ref _PHYSICAL_MEMORY_OFFSET: Mutex<usize> = Mutex::new(0);
    static ref PHYSICAL_MEMORY_OFFSET: usize = *_PHYSICAL_MEMORY_OFFSET.lock();
}

lazy_static! {
    static ref PAGE_ALLOCATOR: Mutex<PageAllocator> = Mutex::new(PageAllocator::new());
}

pub fn init(boot_info: &'static BootInfo) {
    // This is done exactly once, before anyone has accessed PHYSICAL_MEMORY_OFFSET,
    // creating an immutable value we can set at runtime.
    *_PHYSICAL_MEMORY_OFFSET.lock() = boot_info.physical_memory_offset as usize;
    // available_frames is a global bootstrap of physical memory pages.
    // - On first iteration of frame_allocator::usable_frames, every frame is guaranteed to be unused
    //   physical memory and safe to map to pages.
    // - If it is ever used anywhere else it is unsafe, as any yielded frames may already or in the future
    //   be mapped by the global frame allocator.
    //   - Doing anything with these frames is already marked unsafe, and the function itself is
    //     implemented in safe code, so it is "safe" despite these caveats.
    // - Any frames yielded must either be semantically &'static, or be manually passed to
    //   FRAME_ALLOCATOR.dealloc(frame) so that it may reuse them.
    //   - I should eventually find a way to encode this in the type system
    let mut available_frames = frame_allocator::usable_frames(&boot_info.memory_map);
    let mut allocated_frames: usize = 0;
    unsafe {
        allocator::init_kernel_heap(&mut || {
            allocated_frames += 1;
            available_frames
                .next()
                .expect("Failed to allocate frame during kernel heap init")
        });
    };
    // Now that the bootstrap allocator is initialized, we can start doing more complicated things!
    // Let's initialize our arena-based page allocator.
    unsafe {
        (*PAGE_ALLOCATOR.lock()).init(&boot_info.memory_map, allocated_frames);
    };
}

bitflags! {
    pub struct PageFaultError: u32 {
        const PRESENT = 1;
        const WRITE = 1 << 1;
        const USER_MODE = 1 << 2;
        const RESERVED_WRITE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
        const PROTECTION_KEY = 1 << 5;
        const SHADOW_STACK = 1 << 6;
        // 7-14 reserved
        const SOFTWARE_GUARD_EXTENSION = 1 << 15;
        // reserved
    }
}

// Doesn't need to be unsafe because casting the pointer to anything
// is already unsafe
#[inline]
fn physical_to_virtual(address: usize) -> usize {
    address + *PHYSICAL_MEMORY_OFFSET
}

// I actually really like the x86_64 VirtAddr/PhysAddr types, TODO to
// refactor the whole kernel on top of similar ideas
pub fn translate_virtual_address(address: usize) -> Result<usize, Err> {
    let [l4_index, l3_index, l2_index, l1_index] = [
        (address >> (9 * 3) + 12) & 0x1FF,
        (address >> (9 * 2) + 12) & 0x1FF,
        (address >> (9 * 1) + 12) & 0x1FF,
        (address >> (9 * 0) + 12) & 0x1FF,
    ];
    let l4_table = unsafe { page_table::l4::PageTable::get() };
    let l3_table = l4_table[l4_index].deref()?;
    let l2_table = l3_table[l3_index].deref()?;
    let l1_table = l2_table[l2_index].deref()?;
    let l1_entry = &l1_table[l1_index];
    let _memory_block = l1_entry.deref()?;
    Ok(l1_entry.pointer() + (address & 0xFFF))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_case]
    fn test_vga_buffer_identity_page_mapping() {
        assert_eq!(0xb8000, translate_virtual_address(0xb8000).unwrap());
    }

    #[allow(dead_code)] //#[test_case]  // huge page support not implemented yet
    fn test_physical_adress_offset_maps_to_0() {
        assert_eq!(
            0,
            translate_virtual_address(*PHYSICAL_MEMORY_OFFSET).unwrap()
        );
    }

    #[test_case]
    fn test_page_not_present() {
        match translate_virtual_address(0xdeadbeef) {
            Err(Err::PageNotPresent) => (),
            _ => panic!("Expected 0xdeadbeef to not be mapped"),
        }
    }

    // TODO: test invlpg for updated pages
    // TODO: huge pages
}
