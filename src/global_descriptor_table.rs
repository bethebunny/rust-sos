use lazy_static::lazy_static;

// Cargo-culted from blog_os
const STACK_SIZE: usize = 4096 * 5;

use x86_64::instructions::segmentation::{Segment, CS};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        // Create a separate stack for handling double faults
        // This prevents triple-faults on stack overflow, which would otherwise cause
        // the double-fault handler to try to load outside a page and page fault
        let mut tss = TaskStateSegment::new();
        // x86_64 crate TSS indexes ISTs by 0; my InterruptTable indexes by 1 (0 is no stack switch)
        tss.interrupt_stack_table[crate::interrupt::DOUBLE_FAULT_STACK - 1] = {
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE
        };
        tss
    };
    static ref GDT: SegmentAccessibleGDT = {
        let mut gdt = GlobalDescriptorTable::new();
        // It's really not clear to me what the code selector does or why I'm setting it here
        // Cargo-culting from blog_os and moving on for now
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        SegmentAccessibleGDT {
            gdt,
            code_selector,
            tss_selector,
        }
    };
}

struct SegmentAccessibleGDT {
    gdt: GlobalDescriptorTable,
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub fn init() {
    GDT.gdt.load();
    unsafe {
        // It really seems like the gdt.load() (or the CPU) should be doing this for me
        // eg. add_entry could be polymorphic on the segment and register loader fns for each
        CS::set_reg(GDT.code_selector);
        load_tss(GDT.tss_selector);
    };
}

// Code below WIP replacement of GlobalDescriptorTable, I decided it wasn't worth it.
// - The things I'm interested in doing with this OS probably won't dive deep into exceptions
// - If they did, the code in the x86_64 library is likely sufficient for what I'd need
// - Anything I build even in the medium term would likely just be a worse version of the x86_64 code

// lazy_static! {
//     static ref TASK_STATE_SEGMENT: TaskStateSegment = {
//         let mut tss = TaskStateSegment::new();
//         tss.interrupt_stack_table[DOUBLE_FAULT_STACK] = {
//             // TODO: do we need any protection against stack overflow in double fault?
//             // TODO: can we avoid initializing this?
//             static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

//             let stack_start = &STACK as *const _ as usize;
//             // x86 stacks grow downwards, so return a pointer to the end of the stack
//             (stack_start + STACK_SIZE) as u64
//         };
//         tss
//     };

//     static ref GLOBAL_DESCRIPTOR_TABLE: GlobalDescriptorTable = {
//         let mut gdt = GlobalDescriptorTable::new();
//         gdt.push(TASK_STATE_SEGMENT.pointer());
//         gdt
//     };
// }

// pub fn init() {
//     GLOBAL_DESCRIPTOR_TABLE.load();
// }

// // https://en.wikipedia.org/wiki/Task_state_segment
// // We currently only use this for storing stack pointers to interrupt stacks
// #[repr(C, packed)]
// pub struct TaskStateSegment {
//     reserved_0: u32,
//     // Pointers to privilege level stacks
//     pub privilege_stack_table: [u64; 3],
//     reserved_1: u64,
//     // Pointers to interrupt stacks
//     pub interrupt_stack_table: [u64; 7],
//     reserved_2: u64,
//     reserved_3: u16,
//     pub io_map_base_address: u16,
// }

// impl TaskStateSegment {
//     pub fn new() -> Self {
//         TaskStateSegment {
//             reserved_0: 0,
//             privilege_stack_table: [0; 3],
//             reserved_1: 0,
//             interrupt_stack_table: [0; 7],
//             reserved_2: 0,
//             reserved_3: 0,
//             io_map_base_address: 0,
//         }
//     }

//     fn pointer(&self) -> u64 {
//         self as *const _ as u64
//     }
// }

// #[derive(Debug)]
// pub struct GlobalDescriptorTable {
//     table: [u64; 8],
//     length: usize,
// }

// impl GlobalDescriptorTable {
//     pub fn new() -> Self {
//         GlobalDescriptorTable {
//             table: [0; 8],
//             length: 1,
//         }
//     }

//     fn push(&mut self, pointer: u64) {
//         self.table[self.length] = pointer;
//         self.length += 1
//     }

//     pub fn load(&'static self) {
//         let pointer = TablePointer {
//             table_raw_pointer: &self.table as *const _ as u64,
//             table_limit: (size_of::<u64>() * self.length - 1) as u16,
//         };
//         unsafe { asm!("lgdt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags)) };
//     }
// }
