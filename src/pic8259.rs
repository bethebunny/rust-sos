use core::arch::asm;
use spin::Mutex;

use crate::{
    interrupt::table::Interrupt,
    serial::{port_read_byte, port_write_byte},
};

pub const PIC_INTERRUPT_OFFSET: u8 = 32;

pub static PIC: Mutex<ChainedPIC> = Mutex::new(ChainedPIC::new(PIC_INTERRUPT_OFFSET));

const BASE_PIC_COMMAND_PORT: u16 = 0x20;
const CHAINED_PIC_COMMAND_PORT: u16 = 0xA0;

const WAIT_PORT: u16 = 0x80;
const PIC_COMMAND_INIT: u8 = 0x11;
const PIC_COMMAND_END_OF_INTERRUPT: u8 = 0x20;
const PIC_MODE_8086: u8 = 0x01;

pub fn init() {
    unsafe { PIC.lock().init() };
    // enable hardware interrupts
    unsafe { asm!("sti", options(nomem, nostack)) };
}

// Comment shamelessly taken from crate pic8259.
// We need to add a delay between writes to our PICs, especially on
// older motherboards.  But we don't necessarily have any kind of
// timers yet, because most of them require interrupts.  Various
// older versions of Linux and other PC operating systems have
// worked around this by writing garbage data to port 0x80, which
// allegedly takes long enough to make everything work on most
// hardware.  Here, `wait` is a closure.
unsafe fn wait() {
    port_write_byte(WAIT_PORT, 0);
}

struct PIC {
    interrupt_offset: u8,
    command_port: u16,
    data_port: u16,
}

enum PICChainMode {
    Base = 4,
    Chained = 2,
}

impl PIC {
    const fn new(interrupt_offset: u8, command_port: u16) -> Self {
        PIC {
            interrupt_offset,
            command_port,
            data_port: command_port + 1,
        }
    }

    unsafe fn init(&self, chain_mode: PICChainMode) {
        // Save mask to restore after init
        let mask: u8 = port_read_byte(self.data_port);
        // Signal a 3 byte initialization sequence for the controller
        // - Byte 1: set interrupt offset
        // - Byte 2: set chaining mode
        // - Byte 3: Set controller mode
        // Trigger a wait in between each.
        port_write_byte(self.command_port, PIC_COMMAND_INIT);
        wait();
        port_write_byte(self.data_port, self.interrupt_offset);
        wait();
        port_write_byte(self.data_port, chain_mode as u8);
        wait();
        port_write_byte(self.data_port, PIC_MODE_8086);
        wait();
        // Re-set mask
        port_write_byte(self.data_port, mask);
    }

    fn interrupt_in_range(&self, interrupt: u8) -> bool {
        (self.interrupt_offset..self.interrupt_offset + 8).contains(&interrupt)
    }

    unsafe fn signal_end_of_interrupt(&self) {
        port_write_byte(self.command_port, PIC_COMMAND_END_OF_INTERRUPT);
    }
}

pub struct ChainedPIC {
    base_pic: PIC,
    chained_pic: PIC,
}

// Handles 15 interrupts in chained mode
impl ChainedPIC {
    const fn new(interrupt_offset: u8) -> Self {
        ChainedPIC {
            base_pic: PIC::new(interrupt_offset, BASE_PIC_COMMAND_PORT),
            chained_pic: PIC::new(interrupt_offset + 8, CHAINED_PIC_COMMAND_PORT),
        }
    }

    // Unsafe if struct is misconfigured, eg. bad interrupt offsets
    pub unsafe fn init(&self) {
        self.base_pic.init(PICChainMode::Base);
        self.chained_pic.init(PICChainMode::Chained);
    }

    // Safety: must only be called from the interrupt handler for Interrupt
    pub unsafe fn notify_end_of_interrupt(&self, interrupt: Interrupt) {
        let interrupt = interrupt as u8;
        if self.chained_pic.interrupt_in_range(interrupt) {
            self.chained_pic.signal_end_of_interrupt();
            self.base_pic.signal_end_of_interrupt();
        } else if self.base_pic.interrupt_in_range(interrupt) {
            self.base_pic.signal_end_of_interrupt();
        } else {
            panic!("Notified end of unhandled interrupt");
        }
    }
}
