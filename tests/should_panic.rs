#![no_std]
#![no_main]

use core::panic::PanicInfo;

use sos::{serial_print, serial_println, test_runner_exit, QemuExitStatus};

// TODO: It _should_ be possible to implement a test handler that can test
// panics with core::intrinsics::r#try and some nice interface like
// #[test_case]
// fn test_f_panics() {
//     assert_panic!(f(), message="blah");
// }
// I wasn't able to get core::intrinsics::r#try to actually work
// to avoid a panic. There's something related to eh_personality that I don't
// understand that prevents core::intrinsics::r#try from doing its thing.
// Followup later looking at the `unwinding` crate and pulling in a minimal
// set of things can implement enough panic implementation for core::intrinsics::r#try.

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_panic();
    serial_println!("[test did not panic]");
    test_runner_exit(QemuExitStatus::Failed);
}

fn test_panic() {
    serial_print!("should_panic::test_panic...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    test_runner_exit(QemuExitStatus::Success);
}
