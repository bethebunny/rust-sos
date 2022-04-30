#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(sos::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use alloc::boxed::Box;
use core::panic::PanicInfo;

use bootloader::BootInfo;
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

bootloader::entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    sos::init(&boot_info);
    // ('a'..'z').for_each(|c| println!("{}", c));
    print!("{}", 'H');
    print!("ello ");
    print!("Wörld!");
    println!();
    println!("The numbers are {} and {}", 42, 1.0 / 3.0);

    // let l4_table = unsafe { sos::memory::page_table::l4::PageTable::get() };
    // for (i, l3_table) in l4_table.iter().enumerate() {
    //     // This doesn't feel super safe. It's kinda cool, but can _easily_ lead to accidental panics
    //     // if you don't realize you're calling deref implicitly.
    //     if l3_table.present() {
    //         println!("{:0>3}: {:?}", i, l3_table);
    //         for (i, l2_table) in l3_table.iter().enumerate() {
    //             if l2_table.present() {
    //                 println!(" {:0>3}: {:?}", i, l2_table);
    //                 // for (i, l1_table) in l2_table.iter().enumerate() {
    //                 //     if l1_table.present() {
    //                 //         println!("  {}: {:?}", i, l1_table);
    //                 //         for (i, entry) in l1_table.iter().enumerate() {
    //                 //             if entry.present() {
    //                 //                 println!("   {}: {:?}", i, entry);
    //                 //             }
    //                 //         }
    //                 //     }
    //                 // }
    //             }
    //         }
    //     }
    // }

    let some_addresses = [
        // the identity-mapped vga buffer page
        0xb8000,
        // some code page
        0x201008,
        // some stack page
        0x0100_0020_1a10,
        // virtual address mapped to physical address 0
        // - This currently does something nonsense and unsafe because we
        //   haven't implemented huge pages!
        boot_info.physical_memory_offset,
        // should be a page fault
        0,
        0xdeadbeef,
    ];
    for virtual_address in some_addresses {
        match sos::memory::translate_virtual_address(virtual_address) {
            Ok(physical_address) => println!(
                "Virtual({:#x}) -> Physical({:#x})",
                virtual_address, physical_address
            ),
            Err(error) => println!(
                "Virtual({:#x}) lookup failed: {:#?}",
                virtual_address, error,
            ),
        }
    }
    println!("Badger");
    use sos::memory::page_table;
    let l4_table = unsafe { page_table::l4::PageTable::get() };
    l4_table[0][0][0][0] = page_table::l1::PageTableEntry::new(0xb8063);
    let some_ptr: u64 = 0x400;
    let vol: *mut u64 = some_ptr as *const u64 as *mut u64;
    unsafe { *vol = 0x_f021_f077_f065_f04e };

    let x = Box::new(42);
    println!("x: {}", *x);
    println!("&x: {:p}", x);

    // let slab_alloc = sos::memory::allocator::SlabAllocator::new(sos::ALLOCATOR);

    // TODO: hmm .unwrap() causes something _like_ a panic but not an actual panic?
    // use core::arch::asm;
    // unsafe { asm!("div ecx", in("edx") 0, in("eax") 42, in("ecx") 0) };
    // let mut x;
    // unsafe { x = *(0 as *mut u64) };
    // println!("{}", x);
    // unsafe { *(0xdeadbeef as *mut u64) = 42 };
    // TODO: tests for double page faults, stack overflows
    // fn overflow() {
    //     overflow();
    // }
    // overflow();
    // unsafe { *(0xdeadbeef as *mut u64) = 42 };
    // use core::arch::asm;
    // unsafe { asm!("div ecx", in("edx") 0, in("eax") 42, in("ecx") 0) };

    #[cfg(test)]
    test_main();

    // panic!("Kernel shutdown");
    loop {}
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
