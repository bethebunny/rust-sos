use alloc::alloc::Layout;
use core::alloc::{AllocError, Allocator};
use core::ptr::NonNull;

use spin::Mutex;

// pub type MyAllocator = Locked<BumpAllocator>;

// impl MyAllocator {
//     pub const unsafe fn new() -> Self {
//         Locked {
//             value: Mutex::new(BumpAllocator::new()),
//         }
//     }
// }

pub unsafe trait MutAllocator {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);
}

// Assumes that align is a power of 2
pub fn align_up(address: u64, align: u64) -> u64 {
    (address + align - 1) & !(align - 1)
}

static GLOBAL_ALLOCATOR_READY: spin::Mutex<bool> = spin::Mutex::new(false);

pub struct BootstrapAllocator<A: Allocator> {
    bootstrapped: bool,
    allocated: Option<NonNull<[u8]>>,
    allocator: A,
}

impl<A: Allocator> BootstrapAllocator<A> {
    pub fn new(allocator: A) -> Self {
        BootstrapAllocator {
            bootstrapped: false,
            allocated: None,
            allocator,
        }
    }
}

unsafe impl<A: Allocator> MutAllocator for BootstrapAllocator<A> {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if self.allocated.is_none() {
            panic!("Bootstrap allocator allocated twice");
        }
        if self.bootstrapped || *GLOBAL_ALLOCATOR_READY.lock() {
            let result = alloc::alloc::Global.allocate(layout)?;
            self.allocated = Some(result);
            self.bootstrapped = true;
            Ok(result)
        } else {
            let result = self.allocator.allocate(layout)?;
            self.allocated = Some(result);
            Ok(result)
        }
    }
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) {
        if self.bootstrapped {
            alloc::alloc::Global.deallocate(ptr, layout);
        } else {
            self.allocator.deallocate(ptr, layout);
        }
        self.allocated = None;
    }
}

impl<A: Allocator + Clone> Clone for BootstrapAllocator<A> {
    fn clone(&self) -> Self {
        BootstrapAllocator {
            bootstrapped: self.bootstrapped,
            allocated: None,
            allocator: self.allocator.clone(),
        }
    }
}

// I don't really want to be defining this everywhere :/ I need to come up with
// a better pattern for implementing Sync for allocators.
pub struct Locked<T> {
    value: Mutex<T>,
}

impl<T> Locked<T> {
    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.value.lock()
    }
}

unsafe impl<A: Allocator> Allocator for Locked<BootstrapAllocator<A>> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.lock().deallocate(ptr, layout)
    }
}
