use alloc::alloc::{GlobalAlloc, Layout};
use alloc::boxed::Box;
use core::alloc::{AllocError, Allocator};
use core::hash::Hash;
use core::ops::Range;
use core::ptr::{addr_of_mut, null_mut, NonNull};

use hashbrown::hash_map::HashMap;
use spin::Mutex;

use super::{KERNEL_HEAP_SIZE, KERNEL_HEAP_START};
use crate::collections::{DoublyLinkedList, DoublyLinkedListNode};

pub unsafe trait MutAllocator {
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;
    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout);
}

pub struct BumpAllocator {
    heap_start: u64,
    heap_size: u64,
    next: u64,
    allocations: usize,
}

impl BumpAllocator {
    pub const unsafe fn new() -> Self {
        BumpAllocator {
            heap_start: KERNEL_HEAP_START,
            heap_size: KERNEL_HEAP_SIZE,
            next: KERNEL_HEAP_START,
            allocations: 0,
        }
    }

    pub fn upper_bound(&self) -> u64 {
        self.heap_start + self.heap_size
    }
}

// Assumes that align is a power of 2
fn align_up(address: u64, align: u64) -> u64 {
    (address + align - 1) & !(align - 1)
}

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
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {}
}

impl BumpAllocator {
    fn alloc(&mut self, layout: Layout) -> *mut [u8] {
        let requested = align_up(self.next, layout.align() as u64);
        let next = requested + layout.size() as u64;
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

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.value.lock().alloc(layout).as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.value.lock().dealloc(ptr, layout)
    }
}

struct StaticLinkedListNode {
    next: Option<&'static mut StaticLinkedListNode>,
}

impl StaticLinkedListNode {
    pub const fn new() -> Self {
        StaticLinkedListNode { next: None }
    }
}

// S must be >= 8, but I don't know how to require that via the type.
// If S < 8 (aka word size in bytes) then it won't have space to contain
// the linked list nodes that are used to track deallocated space.
pub struct FixedSizeAllocatorBlock<const S: usize> {
    head: Option<&'static mut StaticLinkedListNode>,
    next_never_allocated: *mut [u8; S],
    block_end: *mut [u8; S],
    num_allocated: usize,
}

impl<const S: usize> FixedSizeAllocatorBlock<S> {
    // Safety: block must be a range of data (eg. a virtual page) that
    // is safe for the allocator to allocate to objects
    pub const unsafe fn new(block: Range<*mut u8>) -> Self {
        debug_assert!(core::mem::size_of::<StaticLinkedListNode>() == 8);
        debug_assert!(S >= core::mem::size_of::<StaticLinkedListNode>());
        FixedSizeAllocatorBlock {
            head: None,
            next_never_allocated: block.start as *mut [u8; S],
            block_end: block.end as *mut [u8; S],
            num_allocated: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.num_allocated == 0
    }

    pub fn is_full(&self) -> bool {
        self.head.is_none() && self.next_never_allocated >= self.block_end
    }
}

unsafe impl<const S: usize> MutAllocator for FixedSizeAllocatorBlock<S> {
    // We just punt on alignment :/ don't as for aligned data from this allocator
    // (unless align <= layout.size() and both are multiples of 2!)
    fn allocate(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(layout.size() <= S);
        debug_assert!(layout.align() <= layout.size());
        match &mut self.head {
            Some(ref mut head) => {
                let ptr = addr_of_mut!(*head) as *mut [u8; S];
                self.head = head.next.take();
                self.num_allocated += 1;
                unsafe { Ok(NonNull::new_unchecked(ptr)) }
            }
            None => {
                let next = unsafe { self.next_never_allocated.offset(1) };
                if self.block_end >= self.next_never_allocated {
                    let ptr = self.next_never_allocated;
                    self.next_never_allocated = next;
                    self.num_allocated += 1;
                    unsafe { Ok(NonNull::new_unchecked(ptr)) }
                } else {
                    Err(AllocError)
                }
            }
        }
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, _layout: Layout) {
        // We can be sure of alignment and size because we never allocate fewer than 64 bytes
        let ptr = ptr.as_ptr() as *mut StaticLinkedListNode;
        let mut node = StaticLinkedListNode::new();
        node.next = self.head.take();
        ptr.write(node);
        self.head = Some(&mut *ptr);
    }
}

// impl FixedSizeAllocator {
//     pub const unsafe fn new() -> Self {
//         FixedSizeAllocator {
//             head64: FixedLinkedListAllocatorNode::<64>::new(),
//             backup_allocator: BumpAllocator::new(),
//         }
//     }

//     fn alloc(&mut self, layout: Layout) -> *mut [u8] {
//         if layout.size() <= 64 {
//             match &mut self.head64.next {
//                 Some(ref mut node) => {
//                     let ptr = addr_of_mut!(*node) as *mut u8;
//                     self.head64.next = node.next.take();
//                     core::ptr::slice_from_raw_parts_mut(ptr, layout.size())
//                 }
//                 None => self.backup_allocator.alloc(layout),
//             }
//         } else {
//             self.backup_allocator.alloc(layout)
//         }
//     }

//     fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
//         // We can be sure of alignment and size because we never allocate fewer than 64 bytes
//         if layout.size() <= 64 {
//             let ptr = ptr as *mut FixedLinkedListAllocatorNode<64>;
//             let mut node = FixedLinkedListAllocatorNode::<64>::new();
//             node.next = self.head64.next.take();
//             unsafe { ptr.write(node) };
//             unsafe { self.head64.next = Some(&mut *ptr) };
//         }
//     }
// }

// pub enum CircularlyLinkedList<T: 'static> {
//     Empty,
//     Head(DoublyLinkedListNode<T>),
// }

// trait ObjectAllocator<T, A> where A: SlabAllocator<core::mem::size_of::<T>> {
//     fn allocate(&mut self) -> T;
//     fn deallocate(&mut self, T);
// }

// We should use a Page allocator for the blocks, and a separate typed slab allocator for the doubly linked lists
pub struct SlabAllocator<const S: usize, PA: Allocator + Clone, SA: Allocator + Clone> {
    available: DoublyLinkedList<SlabAllocatorBlock<S>, SA>,
    full: DoublyLinkedList<SlabAllocatorBlock<S>, SA>,
    page_allocator: PA,
}

impl<const S: usize, PA: Allocator + Clone, SA: Allocator + Clone> SlabAllocator<S, PA, SA> {
    pub fn new(page_allocator: PA, slab_allocator: SA) -> Self {
        SlabAllocator {
            available: DoublyLinkedList::new_in(slab_allocator.clone()),
            full: DoublyLinkedList::new_in(slab_allocator.clone()),
            page_allocator,
        }
    }

    pub fn allocate(&mut self) -> Result<NonNull<[u8; S]>, AllocError> {
        let slab = match self.available.head {
            Some(ref mut head) => &mut head.value,
            None => {
                let slab_ptr = self
                    .page_allocator
                    // TODO: this isn't the right size anymore
                    .allocate(Layout::new::<[[u8; S]; 64]>())?;
                let slab = SlabAllocatorBlock::new(slab_ptr.as_mut_ptr() as *mut [[u8; S]; 64]);
                self.available.append(slab);
                &mut self.available.head.as_mut().unwrap().value
            }
        };
        let result = NonNull::new(slab.allocate()).ok_or(AllocError);
        // slab is always self.available.head
        if slab.full() {
            self.full.append(self.available.pop_front().unwrap());
        }
        result
    }

    pub fn deallocate(&mut self, ptr: NonNull<[u8; S]>) {
        // Need to be able to map ptr back to its slab somehow, but I don't know how :(
        // Ideas:
        // - Have a mapping from range -> slab reference? IDK if that even works because of
        //   the nonsense where sometimes I have a heap pointer to a slab and sometimes I don't
        //   - Yikes yikes yikes
        // - Worst case I can iterate over all the slabs and find it :P
        // - Maybe reconsider storing the flags alongside the data again?
        //   - Not even ideal because I don't have a good idea of how I want to figure out the current index anyway!
        todo!();
    }
}

// This would be a lot easier if we had a bootstrap allocator.
// Since we don't, we need to be very careful about representing the structures
// and memory allocation we _do_ have.
// impl<T> CircularlyLinkedList<T> {
//     // pub fn append(&mut self, value: T, allocator: impl Allocator) {
//     //     match self {
//     //         Empty => {
//     //             let node = DoublyLinkedListNode::new(value);
//     //             *self = CircularlyLinkedList::Head(node)
//     //         }
//     //         CircularlyLinkedList::Head(node) => {
//     //             let new_node = DoublyLinkedListNode {
//     //                 next: Some(node),
//     //                 prev: node.prev.take(),
//     //                 value: value,
//     //             };
//     //             if let Some(ref mut prev) = node.prev {
//     //                 prev.next = Some(&new_node)
//     //             }
//     //         }
//     //     }
//     // }

//     // // I don't even know what the semantics are here :/
//     // pub fn pop(&mut self) {}
// }

// TODO: what to do about alignment for align > size?
// TODO: include doubly linked list pointers either as a wrapper or internally
// TODO: how to know which slabs have available allocations?
//  - 2 double linked lists, 1 for full allocators and 1 for available ones
//  - on free, if the slab is full, put it at the back of the "available" list
pub struct SlabAllocatorBlock<const S: usize> {
    allocated_bit_indices: u64,
    // data: [T; 64],
    data: *mut [[u8; S]],
}

impl<const S: usize> SlabAllocatorBlock<S> {
    pub fn new(data: *mut [[u8; S]; 64]) -> Self {
        SlabAllocatorBlock {
            allocated_bit_indices: 0,
            data,
        }
    }

    #[inline]
    pub fn allocate(&mut self) -> *mut [u8; S] {
        let next_available = self.allocated_bit_indices.trailing_ones();
        self.allocated_bit_indices &= 1 << next_available;
        unsafe { self.data.as_mut_ptr().offset(next_available as isize) }
    }

    #[inline]
    pub fn full(&self) -> bool {
        !self.allocated_bit_indices == 0
    }

    #[inline]
    pub fn empty(&self) -> bool {
        self.allocated_bit_indices == 0
    }

    #[inline]
    pub fn deallocate(&mut self, ptr: *mut [u8]) {
        let ptr = ptr as *mut [u8; S];
        let index = unsafe { self.data.as_mut_ptr().offset_from(ptr) };
        self.allocated_bit_indices &= !(1 << index);
    }
}

// pub struct SlabAllocator<'a, T> {
//     available_slabs: CircularlyLinkedList<'a, SlabAllocatorBlock<T>>,
//     full_slabs: CircularlyLinkedList<'a, SlabAllocatorBlock<T>>,
// }

// impl<T> SlabAllocator<'static, T> {
//     // data should be of size 64*sizeof(T)
//     pub unsafe fn new(data: *mut T) -> Self {
//         SlabAllocator {
//             available_slabs: CircularlyLinkedList::new(SlabAllocatorBlock::new(data)),
//             full_slabs: CircularlyLinkedList::Empty,
//         }
//     }

//     pub fn allocate(&mut self) -> *mut T {
//         let slab = match self.available_slabs {
//             CircularlyLinkedList::Empty => {
//                 let slab = self.slab_allocator.allocate_slab<T>();
//                 self.available_slabs = CircularlyLinkedList::new(slab);
//                 slab
//             },
//             CircularlyLinkedList::Head(node) => node.value,
//         };
//         let ptr = slab.allocate();
//         if slab.is_empty() {
//             self.available_slabs.pop();
//             self.full_slabs.append(slab);
//         }
//         ptr
//     }

//     fn slab_node_for_ptr(&self, ptr: *mut T) -> &CircularlyLinkedList<SlabAllocatorBlock<T>> {
//         todo!();
//     }

//     pub fn deallocate(&mut self, ptr: *mut T) {
//         let node = self.slab_node_for_ptr(ptr);
//         let mut slab = &node.value;
//         if slab.full() {
//             node.pop();
//             self.available_slabs.append(slab);
//         }
//         slab.deallocate(ptr);
//     }
// }

// // TODO: have a bootstrap allocator which can be used to implement heap values for
// // the real allocator
// pub struct FixedBlockSlabAllocator {
//     // TODO: can we make this configurable, eg. SLAB_SIZES = [8, 16, 32, ]
//     // We maybe could with Boxes
//     slab8: SlabAllocator<u8>,
//     slab16: SlabAllocator<u16>,
//     slab32: SlabAllocator<u32>,
//     // 64 * 64 is one page, meaning this data structure is a bit bigger than a page :/
//     // I honestly think I'm working too hard on this design rn. Having the bits be co-located
//     // with the data is turning out to be a _lot_ of trouble and messes with alignment, so we
//     // shouldn't do it.
//     slab64: SlabAllocator<u64>,
//     // For larger values we should use different mechanisms.
//     // For 128 -> (4096 - 128) we use a linked list + splitting approach
//     // For >=(4096 - 128) we allocate memory pages
//     // For >= 1mb we allocate large pages if possible
// }

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

unsafe impl<A: Allocator> Allocator for Locked<BootstrapAllocator<A>> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.lock().allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.lock().deallocate(ptr, layout)
    }
}

pub type MyAllocator = Locked<BumpAllocator>;

impl MyAllocator {
    pub const unsafe fn new() -> Self {
        Locked {
            value: Mutex::new(BumpAllocator::new()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::boxed::Box;
    #[allow(dead_code)] // #[test_case]  TODO: slow
    fn many_boxes() {
        for i in 0..KERNEL_HEAP_SIZE {
            let v = Box::new(i);
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
        let alloc: Locked<BumpAllocator> = unsafe { MyAllocator::new() };
        let mut v1 = Vec::<u64, &MyAllocator>::new_in(&alloc);
        let mut v2 = Vec::<u64, &MyAllocator>::new_in(&alloc);
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
        let alloc: Locked<BumpAllocator> = unsafe { MyAllocator::new() };
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

    #[test_case]
    fn custom_allocator() {
        let alloc: Locked<BumpAllocator> = unsafe { MyAllocator::new() };
        let mut v1 = Vec::<u64, &MyAllocator>::new_in(&alloc);
        let mut v2 = Vec::<u64, &MyAllocator>::new_in(&alloc);
        let n = 1000;
        for i in 0..n {
            v1.push(i);
            v2.push(i);
        }
        assert_eq!(v1.iter().sum::<u64>(), (n - 1) * n / 2);
        assert_eq!(v2.iter().sum::<u64>(), (n - 1) * n / 2);
    }
}
