#![no_std]
#![cfg_attr(test, no_main)] // why can't we always just no_main?
#![feature(alloc_error_handler)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

extern crate alloc;

pub mod global_descriptor_table;
pub mod interrupt;
pub mod keyboard;
pub mod memory;
pub mod pic8259;
pub mod serial;
pub mod vga_buffer;

use core::panic::PanicInfo;

use bootloader::BootInfo;

pub fn init(boot_info: &'static BootInfo) {
    memory::init(boot_info);
    global_descriptor_table::init();
    interrupt::init();
    pic8259::init();
}

#[global_allocator]
static ALLOCATOR: memory::allocator::Allocator = unsafe { memory::allocator::Allocator::new() };

#[alloc_error_handler]
fn alloc_error_handler(layout: alloc::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

const IOBASE_PORT: u16 = 0xF4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum QemuExitStatus {
    Success = 0x10,
    Failed = 0x11,
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) -> ! {
    serial_println!("Running {} tests", tests.len());
    tests.iter().for_each(|test| test.run());
    test_runner_exit(QemuExitStatus::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    test_runner_exit(QemuExitStatus::Failed);
}

pub fn test_runner_exit(status: QemuExitStatus) -> ! {
    // Write status to IOBASE port
    // exit status will be (status << 1 | 1)
    unsafe { serial::port_write_byte(IOBASE_PORT, status as u8) };
    panic!("Test runner failed to exit");
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info);
}

#[cfg(test)]
bootloader::entry_point!(test_kernel_main);

#[cfg(test)]
fn test_kernel_main(boot_info: &'static bootloader::BootInfo) -> ! {
    init(boot_info);
    test_main();
    loop {}
}
