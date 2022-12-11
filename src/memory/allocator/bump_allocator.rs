use alloc::alloc::Layout;
use core::alloc::AllocError;
use core::ptr::NonNull;

use super::bootstrap_allocator::MutAllocator;

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
}

unsafe impl MutAllocator for BumpAllocator {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let requested = align_up(self.next, layout.align());
        let next = requested + layout.size();
        if next > self.upper_bound() {
            Err(AllocError)
        } else {
            self.next = next;
            self.allocations += 1;
            let ptr = core::ptr::slice_from_raw_parts_mut(requested as *mut u8, layout.size());
            Ok(unsafe { NonNull::new_unchecked(ptr) })
        }
    }

    unsafe fn deallocate(&mut self, _: NonNull<u8>, _: Layout) {
        self.allocations -= 1;
        if self.allocations == 0 {
            self.next = self.heap_start;
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
        let alloc = unsafe { BumpAllocator::new(heap_start, TINY_HEAP_SIZE) }.as_sync();
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
        let alloc = unsafe { BumpAllocator::new(heap_start, HEAP_SIZE) }.as_sync();
        let mut v1 = Vec::<u64, &_>::new_in(&alloc);
        let mut v2 = Vec::<u64, &_>::new_in(&alloc);
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
        let alloc = unsafe { BumpAllocator::new(heap_start, HEAP_SIZE) }.as_sync();
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
