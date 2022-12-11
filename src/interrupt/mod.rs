use core::arch::asm;

use lazy_static::lazy_static;

pub mod table;

use crate::keyboard::{self, Key, KeyboardModifiers};
use crate::memory::PageFaultError;
use crate::{print, println};
use table::{Handler, Interrupt, InterruptStackFrame, InterruptTable};

pub const DOUBLE_FAULT_STACK: usize = 1;

lazy_static! {
    static ref INTERRUPT_TABLE: InterruptTable = {
        let mut table = InterruptTable::empty();
        table.set_handler(
            Interrupt::DivideByZero,
            Handler::Interrupt(divide_by_zero_handler),
        );
        table.set_handler(
            Interrupt::Breakpoint,
            Handler::Interrupt(breakpoint_handler),
        );
        table.set_handler(Interrupt::PageFault, Handler::Exception(page_fault_handler));
        table
            .set_handler(
                Interrupt::DoubleFault,
                Handler::Exception(double_fault_handler),
            )
            .set_stack(DOUBLE_FAULT_STACK as u8);
        table.set_handler(Interrupt::Timer, Handler::Interrupt(timer_handler));
        table.set_handler(Interrupt::Keyboard, Handler::Interrupt(keyboard_handler));
        table
    };
}

#[macro_export]
macro_rules! without_interrupt {
    ($body:block) => {{
        {
            let _guard = $crate::interrupt::DisableInterruptsGuard::guard();
            $body
        }
    }};
}

extern "x86-interrupt" fn divide_by_zero_handler(_: InterruptStackFrame) {
    panic!("div0 :boom:");
}

extern "x86-interrupt" fn breakpoint_handler(_: InterruptStackFrame) {
    println!("breakpoint");
}

extern "x86-interrupt" fn timer_handler(_: InterruptStackFrame) {
    // print!(".");
    unsafe {
        crate::pic8259::PIC
            .lock()
            .notify_end_of_interrupt(Interrupt::Timer);
    };
}

extern "x86-interrupt" fn keyboard_handler(_: InterruptStackFrame) {
    without_interrupt! {{
        let key = match keyboard::KEYBOARD.lock().read_scancode() {
            Some((Key::Character(c, _), modifiers)) if !modifiers.contains(KeyboardModifiers::SHIFT) => Some(c),
            Some((Key::Character(_, c), modifiers)) if modifiers.contains(KeyboardModifiers::SHIFT) => Some(c),
            _ => None,
        };
        if let Some(c) = key {
            print!("{}", c);
        }
    }}
    // print!("k{}", Interrupt::Keyboard as u8);
    unsafe {
        crate::pic8259::PIC
            .lock()
            .notify_end_of_interrupt(Interrupt::Keyboard);
    };
}

extern "x86-interrupt" fn page_fault_handler(frame: InterruptStackFrame, error: u64) {
    println!("Page fault?!");
    let mut invalid_address: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) invalid_address, options(nomem, nostack, preserves_flags))
    };
    println!(
        "PAGE FAULT: Error({:#?}) / ({:#x}) -- {:#?}",
        PageFaultError::from_bits_truncate(error as u32),
        invalid_address,
        frame
    );
    panic!("page fault");
}

extern "x86-interrupt" fn double_fault_handler(frame: InterruptStackFrame, error: u64) {
    println!("DOUBLE FAULT: Error({:#x}) -- {:#?}", error, frame);
    panic!("double fault");
}

pub fn init() {
    println!("Loading interrupt table!");
    println!("{:#?}", INTERRUPT_TABLE[Interrupt::DoubleFault]);
    INTERRUPT_TABLE.load();
}

#[inline]
pub fn are_interrupts_enabled() -> bool {
    // TODO: more general RFlags
    let rflags: u64;
    unsafe { asm!("pushfq; pop {}", out(reg) rflags, options(nomem, preserves_flags)) };
    (rflags & (1 << 9)) != 0
}

pub struct DisableInterruptsGuard {
    reenable: bool,
}

impl DisableInterruptsGuard {
    #[inline]
    pub fn guard() -> Self {
        let guard = DisableInterruptsGuard {
            reenable: are_interrupts_enabled(),
        };
        unsafe { asm!("cli", options(nomem, nostack)) }; // disable interrupts
        guard
    }
}

impl Drop for DisableInterruptsGuard {
    #[inline]
    fn drop(&mut self) {
        if self.reenable {
            unsafe { asm!("sti", options(nomem, nostack)) };
        }
    }
}

#[cfg(test)]
mod test {
    use core::arch::asm;

    #[test_case]
    fn test_breakpoint() {
        unsafe { asm!("int3") };
    }
}
