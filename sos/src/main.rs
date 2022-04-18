#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(sos::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use sos::{print, println};

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    sos::test_panic_handler(info);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    sos::init();
    // ('a'..'z').for_each(|c| println!("{}", c));
    print!("{}", 'H');
    print!("ello ");
    print!("Wörld!");
    println!();
    println!("The numbers are {} and {}", 42, 1.0 / 3.0);

    // use core::arch::asm;
    // unsafe { asm!("div ecx", in("edx") 0, in("eax") 42, in("ecx") 0) };

    #[cfg(test)]
    test_main();

    panic!("Kernel shutdown");
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
