use alloc::alloc::Layout;
use core::alloc::{AllocError, Allocator, GlobalAlloc};
use core::ptr::{null_mut, NonNull};

use spin::Mutex;

pub struct Locked<T> {
    pub value: Mutex<T>,
}

impl<T> Locked<T> {
    pub const fn new(value: T) -> Self {
        Locked {
            value: Mutex::new(value),
        }
    }
    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.value.lock()
    }
}

pub unsafe trait MutAllocator {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);
    fn as_sync(self) -> Locked<Self>
    where
        Self: Sized,
    {
        Locked {
            value: Mutex::new(self),
        }
    }
}

unsafe impl<M: MutAllocator> Allocator for Locked<M> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.lock().deallocate(ptr, layout)
    }
}

unsafe impl<M: MutAllocator> GlobalAlloc for Locked<M> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.lock().allocate(layout) {
            Ok(ptr) => ptr.as_mut_ptr(),
            Err(_) => null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().deallocate(NonNull::new_unchecked(ptr), layout)
    }
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
