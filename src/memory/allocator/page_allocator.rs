use core::ops::Range;
use core::ptr::NonNull;

use bootloader::bootinfo::MemoryMap;
use bootloader::bootinfo::MemoryRegionType;

use super::resource_allocator::ResourceAllocator;
use crate::memory::page_table;
use crate::memory::page_table::l4;
use crate::memory::PAGE_SIZE;

pub struct PageAllocator {
    l4_table: &'static mut l4::PageTable,
    vmem: ResourceAllocator<PAGE_SIZE>,
    pmem: ResourceAllocator<PAGE_SIZE>,
}

const L4_PAGE_SIZE: usize = 1 << 8 << 8 << 8 << 12;

fn l4_page_range(entry_index: usize) -> Range<usize> {
    entry_index * L4_PAGE_SIZE..(entry_index + 1) * L4_PAGE_SIZE
}

impl PageAllocator {
    pub fn new() -> Self {
        let vmem: ResourceAllocator<PAGE_SIZE> = ResourceAllocator::new();
        let pmem: ResourceAllocator<PAGE_SIZE> = ResourceAllocator::new();
        let l4_table = unsafe { l4::PageTable::get() };
        PageAllocator {
            l4_table,
            vmem,
            pmem,
        }
    }

    pub unsafe fn init(&mut self, memory_map: &MemoryMap, used_frames: usize) {
        // Add any non-present l4 pages as available for vmem allocation.
        // If this isn't sufficient, we can go deeper, but iirc only 4 l4 pages are mapped
        // by the bootloader (and maybe 1 more by us for the bootstrap allocator?)
        let l4 = page_table::l4::PageTable::get();
        l4.iter()
            .enumerate()
            .filter(|(_, e)| e.present())
            .for_each(|(i, _)| self.vmem.add(l4_page_range(i)));

        // Add all physical memory regions to the pmem allocator.
        // Assume all used_frames come from the front. We guarantee this with our bootstrap
        // allocator, which iterates over frames in sorted order from MemoryMap.
        let mut to_drop = used_frames;
        let usable_regions = memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        for region in usable_regions {
            let start = region.range.start_frame_number as usize;
            let end = region.range.start_frame_number as usize;
            if end - start > to_drop {
                self.pmem
                    .add((start + to_drop) * PAGE_SIZE..end * PAGE_SIZE);
                to_drop = 0;
            } else {
                to_drop -= end - start;
            }
        }
    }

    // Allocate should allocate contiguous blocks of virtual memory, backed
    // by (not necessarily contiguous) blocks of physical memory.
    // It should intentionally over-allocate virtual memory where possible
    // to allow for fast resizing by just mapping additional pages.
    // pub fn allocate();
    // pub fn allocate_one();
    // pub fn lazy_allocate();
    // pub fn allocate_frame();
    // pub fn allocate_frames();
    // pub fn deallocate();
    // pub fn resize();
    // pub fn to_disk();

    pub fn allocate_frame(&mut self) -> Result<NonNull<[u8]>, ()> {
        // self.allocate_frames(1)
        let start = self.pmem.fast_allocate(1)?.start as *mut u8;
        Ok(unsafe { NonNull::new_unchecked(start as *mut [u8; PAGE_SIZE]) })
    }
    // pub fn allocate_frames(&mut self, frames: usize) -> Result<NonNull<[u8]>, ()> {
    //     let start = self.pmem.fast_allocate(frames)?.start as *mut u8;
    //     Ok(unsafe { NonNull::new_unchecked(start) })
    // }
    pub fn allocate(&mut self, size: usize) -> Result<NonNull<[u8]>, ()> {
        let range = self.vmem.fast_allocate(size)?;
        unsafe {
            // TODO: propagate page allocation error
            let next_frame =
                &mut || self.pmem.fast_allocate(1).unwrap().start as *const () as usize;
            for page in range.clone().step_by(PAGE_SIZE) {
                self.l4_table
                    .map_if_unmapped(page, next_frame)
                    .or(Err(()))?;
            }
        };
        // page_table::l4::PageTable::
        Ok(unsafe {
            NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                range.start as *mut u8,
                range.end,
            ))
        })
    }

    // unsafe fn map_page(&mut self, page: usize) {
    //     self.l4_table
    //         .map_if_unmapped(page, &mut || self.next_frame().unwrap());
    // }
    // fn next_frame(&mut self) -> Result<usize, ()> {
    //     Ok(self.allocate_frame()?.as_ptr() as *const () as usize)
    // }

    pub fn deallocate(&mut self, ptr: *mut u8, size: usize) {
        let start = ptr as usize;
        let range = start..start + size;
        self.vmem.release(range.clone());
        for page in range.step_by(PAGE_SIZE) {
            let entry = unsafe { self.l4_table.unmap(page) };
            let ptr = entry.pointer();
            self.pmem.release(ptr..ptr + PAGE_SIZE);
        }
    }
    // pub fn allocate_frames();
}
