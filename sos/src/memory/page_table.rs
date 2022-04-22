use core::arch::asm;
use core::fmt;
use core::ops::{Deref, DerefMut, Index, IndexMut};
use core::result::Result;
use core::slice::{Iter, IterMut};

macro_rules! page_table {
    ($page_table_name:ident -> $points_to:ty) => {
        pub mod $page_table_name {
            use super::*;

            #[repr(align(4096))]
            pub struct PageTable([PageTableEntry; 512]);
            pub struct PageTableEntry(u64);

            impl PageTableEntry {
                pub fn new(data: u64) -> Self {
                    PageTableEntry(data) // TODO: better API xD
                }
                pub fn pointer(&self) -> u64 {
                    self.0 & 0x000F_FFFF_FFFF_FF000
                }

                pub fn present(&self) -> bool {
                    self.0 & 0x1 != 0
                }

                pub fn deref(&self) -> Result<&$points_to, Err> {
                    if !self.present() {
                        Err(Err::PageNotPresent)
                    } else {
                        Ok(unsafe { &*(crate::memory::physical_to_virtual(self.pointer()) as *mut $points_to) })
                    }
                }

                pub fn deref_mut_or_map(&mut self, next_frame: &mut dyn FnMut() -> u64) -> &mut $points_to {
                    if !self.present() {
                        // TODO: initialize frame to empty page table
                        self.0 = next_frame() | 0x63; // TODO flags
                        crate::println!("Mapped page {:#?}", self);
                    }
                    self.deref_mut()
                }
            }

            // These are _undoubtedly_ a bad idea. Get rid of them ASAP, but playing around
            // with just _how_ bad of an idea they are for now xD
            impl Deref for PageTableEntry {
                type Target = $points_to;
                fn deref(&self) -> &Self::Target {
                    if !self.present() {
                        panic!("Tried to dereference non-present page table");
                    }
                    unsafe { &*(crate::memory::physical_to_virtual(self.pointer()) as *mut Self::Target) }
                }
            }

            impl DerefMut for PageTableEntry {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    if !self.present() {
                        panic!("Tried to dereference non-present page table");
                    }
                    unsafe { &mut *(crate::memory::physical_to_virtual(self.pointer()) as *mut Self::Target) }
                }
            }

            // This is the key safety mechanism for page tables -- We'll see whether this
            // is a good idea or a horrible one. Handle PageTableEntry lifetimes through
            // the Rust compiler, and then call `invlpg` when a PageTableEntry is dropped,
            // ie. if you do *l4_table[0] = l4::PageTableEntry::new() the old page will
            // automatically be invalidated.
            //
            // This could potentially introduce subtle bugs since it's never correct to call
            // invlpg for a still-mapped page table, and rust can drop any owned references
            // on function return. However, page table references are only accessible through
            // unsafe l4::PageTable::get() as an entry point, which has lifetime 'static, we
            // carefully pass those lifetimes through for any usages eg. indexing, so the
            // compiler _should_ know that any references it gets are 'static and not drop them.
            impl Drop for PageTableEntry {
                fn drop(&mut self) {
                    if self.present() {
                        unsafe { asm!("invlpg [{}]", in(reg) self.0) };
                    }
                }
            }

            impl fmt::Debug for PageTableEntry {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.debug_struct(concat!(stringify!($page_table_name), "::PageTableEntry"))
                        .field("pointer", &format_args!("{:#x}", self.pointer()))
                        .field("top_12", &format_args!("{:#x}", self.0 >> 48))
                        .field("options", &format_args!("{:#x}", &self.0 & 0x777))
                        .finish()
                }
            }

            impl PageTable {
                #[allow(dead_code)]
                pub fn iter<'a>(&'a self) -> Iter<'a, PageTableEntry> {
                    self.0.iter()
                }

                #[allow(dead_code)]
                pub fn iter_mut<'a>(&'a mut self) -> IterMut<'a, PageTableEntry> {
                    self.0.iter_mut()
                }
            }

            impl Index<usize> for PageTable {
                type Output = PageTableEntry;
                fn index(&self, index: usize) -> &PageTableEntry {
                    &self.0[index]
                }
            }

            // Safety: PageTableIndex implements Drop which handles invlpg calls
            impl IndexMut<usize> for PageTable {
                fn index_mut(&mut self, index: usize) -> &mut Self::Output {
                    &mut self.0[index]
                }
            }
        }
    };
}

#[repr(align(4096))]
pub struct Memory4KB([u8; 4096]);

page_table!(l1 -> Memory4KB);
page_table!(l2 -> l1::PageTable);
page_table!(l3 -> l2::PageTable);
page_table!(l4 -> l3::PageTable);

impl l4::PageTable {
    pub unsafe fn get() -> &'static mut Self {
        let mut cr3: u64;
        asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        &mut *(crate::memory::physical_to_virtual(cr3 & !0xFFF) as *mut Self)
    }

    // TODO: bigger page sizes
    // Unsafe because
    // TODO: flags
    pub unsafe fn map_if_unmapped(
        &mut self,
        address: u64,
        next_frame: &mut dyn FnMut() -> u64,
    ) -> Result<(), Err> {
        let [l4_index, l3_index, l2_index, l1_index] = [
            (address as usize >> (9 * 3) + 12) & 0x1FF,
            (address as usize >> (9 * 2) + 12) & 0x1FF,
            (address as usize >> (9 * 1) + 12) & 0x1FF,
            (address as usize >> (9 * 0) + 12) & 0x1FF,
        ];
        // Whoops TODO map these to new page entries (how? where do they go in memory?)
        // TODO: flags
        // TODO: why does this work when .deref_or_map requires &mut self?
        self[l4_index] // no wrap
            .deref_mut_or_map(next_frame)[l3_index]
            .deref_mut_or_map(next_frame)[l2_index]
            .deref_mut_or_map(next_frame)[l1_index]
            .deref_mut_or_map(next_frame);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Err {
    PageNotPresent,
    // Don't even have error cases yet for huge pages, we'll probably just fault :P
}
