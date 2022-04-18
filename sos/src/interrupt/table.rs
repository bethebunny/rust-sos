use bitflags::bitflags;
use core::arch::asm;
use core::fmt;
use core::ops::Index;

#[derive(Debug)]
#[repr(C)]
pub struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

bitflags! {
    pub struct EntryOptions: u16 {
        // if all 0, don't switch stacks, otherwis switch to stack 1-7
        const SWITCH_TO_STACK_0 = 1;
        const SWITCH_TO_STACK_1 = 1 << 1;
        const SWITCH_TO_STACK_2 = 1 << 2;
        // 3-7 reserved (unsure for what)
        const INTERRUPTS_ENABLED = 1 << 8;
        // 9-11 must always be 1
        // 12 must always be 0
        const MINIMUM_PRIVILEDGE_LEVEL_0 = 1 << 13;
        const MINIMUM_PRIVILEDGE_LEVEL_1 = 1 << 14;
        // 1 if the table entry is present, otherwise 0
        const PRESENT = 1 << 15;
    }
}

impl EntryOptions {
    const EMPTY: Self = {
        let mut options = EntryOptions::empty();
        options.bits |= 0x7 << 9;
        options
    };
    // pub fn switch_to_stack(&self) -> u8 {
    //     self.bits.bits[0..3] as u8
    // }

    // pub fn set_switch_to_stack(&mut self, stack: u8) -> &mut Self {
    //     self.bits.set_bits(0..3, stack);
    //     self
    // }
}

fn get_current_code_segment() -> u16 {
    let segment: u16;
    unsafe { asm!("mov {0:x}, cs", out(reg) segment, options(nomem, nostack, preserves_flags)) };
    segment
}

// The "x86-interrupt" calling convention does a _lot_ of work for us
// -- similar to "preserve-all", allows the compiler to push any used registers
// onto the stack to avoid function correction. Additionally handles returning via
// iretq, and _I believe_ there's also some builtin compiler complexity related to
// red-zone handling, ie. certain code in the kernel will be compiled with -mno-red-zone
//
// This could be implemented as a trait, but function items are only coercible to traits
// with a single function implementation; otherwise you need to explicitly cast the function
// item to its function pointer type, which is ultimately more awkward.
pub enum Handler {
    Interrupt(extern "x86-interrupt" fn(frame: ExceptionStackFrame)),
    Exception(extern "x86-interrupt" fn(frame: ExceptionStackFrame, error: u64)),
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TableEntry {
    pointer_low: u16,
    global_descriptor_table_selector: u16,
    options: EntryOptions, // u16
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl TableEntry {
    pub fn empty() -> TableEntry {
        TableEntry {
            pointer_low: 0,
            global_descriptor_table_selector: 0,
            options: EntryOptions::EMPTY,
            pointer_middle: 0,
            pointer_high: 0,
            reserved: 0,
        }
    }

    pub fn new(handler: Handler) -> TableEntry {
        let mut entry = Self::empty();
        let pointer = match handler {
            Handler::Interrupt(fp) => fp as u64,
            Handler::Exception(fp) => fp as u64,
        };
        entry.pointer_low = pointer as u16;
        entry.pointer_middle = (pointer >> 16) as u16;
        entry.pointer_high = (pointer >> 32) as u32;
        // TODO: add more behaviors to gdt / privilege levels
        entry.global_descriptor_table_selector = get_current_code_segment();
        entry.options |= EntryOptions::PRESENT;
        entry
    }

    pub fn pointer(&self) -> u64 {
        self.pointer_low as u64
            | (self.pointer_middle as u64) << 16
            | (self.pointer_high as u64) << 32
    }
}

impl fmt::Debug for TableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("pointer", &format_args!("{:#x}", self.pointer()))
            .field("gdt_selector", &self.global_descriptor_table_selector)
            .field("options", &format_args!("{:#x}", &self.options.bits))
            .finish()
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Interrupt {
    DivideByZero = 0,
    Debug = 1,
    NonMaskableInterrupt = 2,
    Breakpoint = 3,
    Overflow = 4,
    BoundRangeExceeded = 5,
    InvalidOpcode = 6,
    DeviceNotAvailable = 7,
    DoubleFault = 8,
    InvalidTss = 9,
    SegmentNotPresent = 10,
    StackSegmentFault = 11,
    GeneralProtectionFault = 12,
    PageFault = 13,
    X87FloatingPoint = 14,
    AlignmentCheck = 15,
    MachineCheck = 16,
    SimdFloatingPoint = 17,
    Virtualization = 18,
    SecurityException = 19,
}

#[derive(Clone, Debug)]
#[repr(C)]
#[repr(align(16))]
pub struct InterruptTable([TableEntry; 256]);

impl Index<usize> for InterruptTable {
    type Output = TableEntry;
    fn index(&self, index: usize) -> &TableEntry {
        return &self.0[index];
    }
}

impl InterruptTable {
    pub fn empty() -> InterruptTable {
        InterruptTable([TableEntry::empty(); 256])
    }

    pub fn set_handler(&mut self, interrupt: Interrupt, handler: Handler) -> &mut EntryOptions {
        self.0[interrupt as usize] = TableEntry::new(handler);
        &mut self.0[interrupt as usize].options
    }

    pub fn load(&'static self) {
        use core::mem::size_of;

        let pointer = InterruptTablePointer {
            table_limit: (size_of::<Self>() - 1) as u16,
            table_raw_pointer: self as *const _ as u64,
        };

        unsafe { asm!("lidt [{}]", in(reg) &pointer, options(readonly, nostack, preserves_flags)) };
    }
}

// packed is critical; without it this struct is aligned to 128 bytes
// while the lidt instruction expects an 80 byte struct
#[repr(C, packed)]
struct InterruptTablePointer {
    table_limit: u16, // table size in bytes - 1
    table_raw_pointer: u64,
}
