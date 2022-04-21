use bitflags::bitflags;
use lazy_static::lazy_static;
use spin::Mutex;

pub mod page_table;

use page_table::Err;

lazy_static! {
    static ref _PHYSICAL_MEMORY_OFFSET: Mutex<u64> = Mutex::new(0);
    static ref PHYSICAL_MEMORY_OFFSET: u64 = *_PHYSICAL_MEMORY_OFFSET.lock();
}

pub fn init(physical_memory_offset: u64) {
    *_PHYSICAL_MEMORY_OFFSET.lock() = physical_memory_offset;
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
fn physical_to_virtual(address: u64) -> u64 {
    address + *PHYSICAL_MEMORY_OFFSET
}

// I actually really like the x86_64 VirtAddr/PhysAddr types, TODO to
// refactor the whole kernel on top of similar ideas
pub fn translate_virtual_address(address: u64) -> Result<u64, Err> {
    let [l4_index, l3_index, l2_index, l1_index] = [
        (address as usize >> (9 * 3) + 12) & 0x1FF,
        (address as usize >> (9 * 2) + 12) & 0x1FF,
        (address as usize >> (9 * 1) + 12) & 0x1FF,
        (address as usize >> (9 * 0) + 12) & 0x1FF,
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
