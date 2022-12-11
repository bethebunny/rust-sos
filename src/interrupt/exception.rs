use core::arch::asm;

#[derive(Debug)]
#[repr(C)]
struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

extern "C" fn something_else(_frame: ExceptionStackFrame) {
    panic!("yay");
}

#[naked]
pub extern "C" fn something() -> ! {
    let mut stack_frame: ExceptionStackFrame;
    unsafe {
        asm!("mov {0:x}, rsp", out(reg) stack_frame, options(nomem, nostack, preserves_flags))
    };
    unsafe { asm!("mov rdi, rsp; call {0:x}", in(reg) something_else) }
    ::core::intrinsics::unreachable();
}
