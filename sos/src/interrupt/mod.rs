use lazy_static::lazy_static;

mod table;

use crate::println;
use table::{ExceptionStackFrame, Handler, Interrupt, InterruptTable};

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
        table
    };
}

extern "x86-interrupt" fn divide_by_zero_handler(_: ExceptionStackFrame) {
    panic!("div0 :boom:");
}

extern "x86-interrupt" fn breakpoint_handler(_: ExceptionStackFrame) {
    println!("breakpoint");
}

pub fn init() {
    println!("Loading interrupt table!");
    INTERRUPT_TABLE.load();
}

#[cfg(test)]
mod test {
    use core::arch::asm;

    #[test_case]
    fn test_breakpoint() {
        unsafe { asm!("int3") };
    }
}
