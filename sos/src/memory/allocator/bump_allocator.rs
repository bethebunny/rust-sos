use alloc::alloc::{GlobalAlloc, Layout};
use core::alloc::{AllocError, Allocator};
use core::ptr::{null_mut, NonNull};

use spin::Mutex;

// TODO this should go somewhere better
// Assumes that align is a power of 2
pub fn align_up(address: usize, align: usize) -> usize {
    (address + align - 1) & !(align - 1)
}

pub struct BumpAllocator {
    heap_start: usize,
    heap_size: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    // TODO: whoops this should really take arguments xD
    // Should be able to initialize this with stack memory for testing!
    pub const unsafe fn new(start: usize, size: usize) -> Self {
        BumpAllocator {
            heap_start: start,
            heap_size: size,
            next: start,
            allocations: 0,
        }
    }

    pub fn upper_bound(&self) -> usize {
        self.heap_start + self.heap_size
    }

    fn alloc(&mut self, layout: Layout) -> *mut [u8] {
        let requested = align_up(self.next, layout.align());
        let next = requested + layout.size();
        if next >= self.upper_bound() {
            core::ptr::slice_from_raw_parts_mut(null_mut(), 0)
        } else {
            self.next = next;
            self.allocations += 1;
            core::ptr::slice_from_raw_parts_mut(requested as *mut u8, layout.size())
        }
    }

    fn dealloc(&mut self, _ptr: *mut u8, _layout: Layout) {
        self.allocations -= 1;
        if self.allocations == 0 {
            self.next = self.heap_start;
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

unsafe impl Allocator for Locked<BumpAllocator> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = self.lock().alloc(layout);
        NonNull::<[u8]>::new(ptr as *mut [u8]).ok_or(AllocError)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.lock().dealloc(ptr.as_ptr(), layout)
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().alloc(layout).as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock().dealloc(ptr, layout)
    }
}

pub type SyncBumpAllocator = Locked<BumpAllocator>;

impl SyncBumpAllocator {
    pub const unsafe fn new(start: usize, size: usize) -> Self {
        Locked {
            value: Mutex::new(BumpAllocator::new(start, size)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::boxed::Box;
    use hashbrown::HashMap;

    const HEAP_SIZE: usize = 25 * 4096;

    #[test_case]
    fn many_boxes() {
        const TINY_HEAP_SIZE: usize = 256;
        let heap: [u8; TINY_HEAP_SIZE] = [0u8; TINY_HEAP_SIZE];
        let heap_start: usize = &heap as *const _ as usize;
        let alloc: Locked<BumpAllocator> =
            unsafe { SyncBumpAllocator::new(heap_start, TINY_HEAP_SIZE) };
        for i in 0..TINY_HEAP_SIZE {
            let v = Box::new_in(i, &alloc);
            assert_eq!(i, *v);
        }
    }

    use alloc::vec::Vec;

    #[test_case]
    fn large_vec() {
        let n = 1000;
        let mut vec = Vec::new();
        for i in 0..n {
            vec.push(i);
        }
        assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
    }

    #[test_case]
    fn custom_allocator() {
        let heap: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
        let heap_start: usize = &heap as *const _ as usize;
        let alloc: Locked<BumpAllocator> = unsafe { SyncBumpAllocator::new(heap_start, HEAP_SIZE) };
        let mut v1 = Vec::<u64, &SyncBumpAllocator>::new_in(&alloc);
        let mut v2 = Vec::<u64, &SyncBumpAllocator>::new_in(&alloc);
        let n = 1000;
        for i in 0..n {
            v1.push(i);
            v2.push(i);
        }
        assert_eq!(v1.iter().sum::<u64>(), (n - 1) * n / 2);
        assert_eq!(v2.iter().sum::<u64>(), (n - 1) * n / 2);
    }

    // count_tts implementation shamelessly pulled from
    // https://danielkeep.github.io/tlborm/book/blk-counting.html
    macro_rules! replace_expr {
        ($_t:tt $sub:expr) => {
            $sub
        };
    }

    macro_rules! count_tts {
        ($($tts:tt)*) => {<[()]>::len(&[$(replace_expr!($tts ())),*])};
    }

    macro_rules! hash_map {
        (via $alloc:expr) => { HashMap::new_in($alloc) };
        ($($k:expr => $v:expr),+ $(,)?; via $alloc:expr) => {
            {
                let mut map = HashMap::with_capacity_in(count_tts!($($k)+), $alloc);
                $(map.insert($k, $v);)+
                map
            }
        };
    }

    #[test_case]
    fn hashmap_custom_allocator() {
        let heap: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
        let heap_start: usize = &heap as *const _ as usize;
        let alloc: Locked<BumpAllocator> = unsafe { SyncBumpAllocator::new(heap_start, HEAP_SIZE) };
        let m1 = hash_map![
            0 => "a",
            1 => "b",
            2 => "c",
            ; via &alloc
        ];
        assert_eq!(m1[&0], "a");
        assert_eq!(m1[&1], "b");
        assert_eq!(m1[&2], "c");
        assert!(!m1.contains_key(&3));
        assert_eq!(3, m1.len());
    }
}
