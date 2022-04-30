use alloc::alloc::{Allocator, Global};
use alloc::format;
use core::ops::Range;
use core::ptr::NonNull;

use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::HashMap;

use crate::collections::hash_map::SimpleBuildHasher;
use crate::collections::{DoublyLinkedList, DoublyLinkedListNode};

/// Based on the VMem resource allocator design described in
/// https://www.usenix.org/legacy/publications/library/proceedings/usenix01/full_papers/bonwick/bonwick.pdf

struct Segment<A = Global>
where
    A: Allocator + Clone,
{
    range: Range<usize>,
    freelist_ptr: Option<NonNull<FreelistNode<A>>>,
}

impl<A: Allocator + Clone> Segment<A> {
    pub fn new(range: Range<usize>) -> Self {
        Segment {
            range,
            freelist_ptr: None,
        }
    }
    pub fn size(&self) -> usize {
        self.range.end - self.range.start
    }

    pub fn split(&mut self, split_point: usize) -> Segment<A> {
        let result = Segment::new(split_point..self.range.end);
        self.range.end = split_point;
        result
    }

    pub fn is_allocated(&self) -> bool {
        self.freelist_ptr.is_none()
    }

    pub fn can_join(&self, other: &Segment<A>) -> bool {
        self.range.end == other.range.start || self.range.start == other.range.end
    }
}

impl<A: Allocator + Clone> core::fmt::Debug for Segment<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Segment")
            .field("range", &self.range)
            .field("allocated", &self.freelist_ptr.is_none())
            .finish()
    }
}

#[derive(Clone, Debug)]
struct SegmentPtr<A: Allocator + Clone = Global>(NonNull<DoublyLinkedListNode<Segment<A>, A>>);

impl<A: Allocator + Clone> Copy for SegmentPtr<A> {}

type Freelist<A> = DoublyLinkedList<SegmentPtr<A>, A>;
type FreelistNode<A> = DoublyLinkedListNode<SegmentPtr<A>, A>;

impl<A: Allocator + Clone> SegmentPtr<A> {
    fn segment(&self) -> &Segment<A> {
        &unsafe { self.0.as_ref() }.value
    }

    fn segment_mut(&mut self) -> &mut Segment<A> {
        &mut unsafe { self.0.as_mut() }.value
    }
}

// Pick M to be floor(log2(max(value)))
// - so for instance if you want to have 2^16 process IDs, choose 16
struct ResourceAllocator<
    const Q: usize = 1,
    A = Global,
    const M: usize = { 8 * core::mem::size_of::<usize>() },
> where
    A: Allocator + Clone,
{
    freelists: [Freelist<A>; M],
    // This needs to be doubly-linked
    // And probably not static, we need to own it
    allocated_segments: HashMap<usize, SegmentPtr<A>, SimpleBuildHasher, A>,
    segments: DoublyLinkedList<Segment<A>, A>,
}

impl<const Q: usize, const M: usize> ResourceAllocator<Q, Global, M> {
    pub fn new() -> Self {
        ResourceAllocator {
            freelists: [(); M].map(|_| Freelist::new()),
            allocated_segments: HashMap::with_hasher(Default::default()),
            segments: DoublyLinkedList::new(),
        }
    }
}
impl<const Q: usize, const M: usize, A: Allocator + Clone> ResourceAllocator<Q, A, M> {
    pub fn new_in(allocator: A) -> Self {
        ResourceAllocator {
            freelists: [(); M].map(|_| Freelist::new_in(allocator.clone())),
            allocated_segments: HashMap::with_hasher_in(Default::default(), allocator.clone()),
            segments: DoublyLinkedList::new_in(allocator.clone()),
        }
    }

    // Assumption: values passed to `add` are always non-adjacent, non-overlapping, and
    // starting past any previous call to add. We can probably come up with a better iterator API
    // to correctly express initialization.
    // For smaller than Q ranges, we can't do anything with them, so they just get leaked.
    pub fn add(&mut self, range: Range<usize>) {
        let segment = Segment::new(range);
        let size = segment.size();
        if size >= Q {
            // Add unallocated segment, and add pointer to correct freelist
            let mut segment_ptr = SegmentPtr(self.segments.append(segment));
            self.coalesce_and_freelist_insert(&mut segment_ptr);
        }
    }

    // - Round size up with ceil_log2 to ensure that an allocated region .size >= size
    // - If best fast freelist is empty, check larger freelists
    // - If all larger freelists are empty, fall back to a linear scan of the next smaller freelist
    fn fast_find_segment(&mut self, size: usize) -> Result<SegmentPtr<A>, ()> {
        // Get the quanta required to allocate at least `size` elements. We'll never allocate zero.
        let qsize = size.div_ceil(Q).max(1);
        // size.log2() rounds down, which is not what we want.
        // each freelist should have ranges of size [2^i, 2^(i+1))
        // (except for the last which can has no upper bound)
        // We want to pick a freelist rounding _up_ to the next power of 2.
        for freelist_idx in ceil_log2(qsize)..M {
            let freelist = &mut self.freelists[freelist_idx];
            if let Ok(segment_ptr) = freelist.pop_front() {
                return Ok(segment_ptr);
            }
        }
        // Still haven't allocated
        // - back up to linear scan of freelist with appropriate size
        let freelist_idx = qsize.log2() as usize;
        if freelist_idx < ceil_log2(qsize) {
            let freelist = &mut self.freelists[freelist_idx];
            let node = freelist
                .find(|segment_ptr| segment_ptr.segment().size() >= size)
                .ok_or(())?;
            unsafe { freelist.remove(node) };
            Ok(unsafe { node.as_ref() }.value)
        } else {
            Err(())
        }
    }

    #[inline]
    fn segment_freelist_idx(&self, segment: &Segment<A>) -> usize {
        // Get the number of quanta that can be stored in the segment. We'll never allocate zero quanta.
        let qsize = segment.size().div_floor(Q);
        debug_assert!(qsize > 0);
        // size.log2() rounds down, which is exactly the criteria we want for adding to freelists;
        // each freelist should have ranges of size [2^i, 2^(i+1)) (except for the last which can
        // has no upper bound)
        (qsize.log2() as usize).min(M)
    }

    fn freelist_insert(&mut self, segment_ptr: &mut SegmentPtr<A>) {
        let freelist = &mut self.freelists[self.segment_freelist_idx(segment_ptr.segment())];
        let freelist_ptr = freelist.insert_front(*segment_ptr);
        segment_ptr.segment_mut().freelist_ptr = Some(freelist_ptr);
    }

    // Safety: SegmentPtr must be a segment we own
    fn freelist_remove(&mut self, segment: &mut Segment<A>) {
        let freelist = &mut self.freelists[self.segment_freelist_idx(segment)];
        // Safety: As long as we own SegmentPtr, we meet the safety requirements of freelist.remove
        // by picking the correct freelist index.
        if let Some(freelist_ptr) = segment.freelist_ptr {
            unsafe { freelist.remove(freelist_ptr) };
            segment.freelist_ptr = None;
        }
    }

    pub fn fast_allocate(&mut self, size: usize) -> Result<Range<usize>, ()> {
        // ignore alignment for now
        // TODO: special case for zero-sized allocations?
        let mut segment_ptr = self.fast_find_segment(size)?;
        segment_ptr.segment_mut().freelist_ptr = None;
        // split the segment if we can and put the new split in the associated freelist
        let qsize = size.div_ceil(Q).max(1);
        let alloc_size = qsize * Q;
        self.try_split_segment(&mut segment_ptr, alloc_size);
        let range = segment_ptr.segment().range.clone();
        self.allocated_segments.insert(range.start, segment_ptr);
        Ok(range)
    }

    fn try_split_segment(&mut self, segment_ptr: &mut SegmentPtr<A>, alloc_size: usize) {
        let segment = segment_ptr.segment_mut();
        let leftover_size = segment.size() - alloc_size;
        // Is there at least enough space for 1*Q left over?
        if leftover_size >= Q {
            let new_segment = segment.split(segment.range.start + alloc_size);
            let mut new_segment_ptr =
                SegmentPtr(self.segments.insert_after(&mut segment_ptr.0, new_segment));
            // insert to the front so that split segments are sometimes allocated at similar times
            // - should reduce framentation in practice; segments of the same "generation" will be co-located
            self.freelist_insert(&mut new_segment_ptr);
        }
    }

    pub fn release(&mut self, range: Range<usize>) {
        // Panic if we're given a range we didn't allocate
        let mut segment_ptr = self.allocated_segments.remove(&range.start).unwrap();
        self.coalesce_and_freelist_insert(&mut segment_ptr);
    }

    fn coalesce_and_freelist_insert(&mut self, segment_ptr: &mut SegmentPtr<A>) {
        // assumption: segment_ptr is not allocated, but not in a freelist yet

        // Otherwise not joinable; this should maybe be a more central concept.
        // The paper keeps track of this so regions which are completely isolated
        // can be freed upstream to a parent allocater, for instance.
        let joinable = |a: &Segment<A>, b: &Segment<A>| {
            a.freelist_ptr.is_some() && b.freelist_ptr.is_some() && a.range.end == b.range.start
        };

        // If prev is joinable and unallocated
        //  - remove prev from its freelist
        //  - delete prev from segments
        //  - update segment_ptr to take ownership of prev's segment
        let prev = &mut unsafe { segment_ptr.0.as_mut() }.prev;
        if let Some(prev) = prev {
            let mut prev = SegmentPtr(*prev);
            let prev_segment = prev.segment_mut();
            let segment = segment_ptr.segment();
            if segment.can_join(prev_segment) && !prev_segment.is_allocated() {
                // prev owns segment_ptr, so remove segment_ptr and then *segment_ptr = prev
                self.freelist_remove(prev_segment);
                unsafe { self.segments.remove(segment_ptr.0) };
                prev_segment.range.end = segment.range.end;
                *segment_ptr = prev;
            }
        }
        // At this point, segment_ptr is still not in a freelist.
        // If next is joinable and unallocated
        //  - remove next from its freelist
        //  - remove segment_ptr from segments
        //  - update next to take ownership of segment_ptr's segment
        //  - add next to a new freelist
        // Otherwise
        //  - add segment_ptr to a freelist
        let next = &mut unsafe { segment_ptr.0.as_mut() }.next;
        if let Some(next) = next {
            let next_segment = &mut next.value;
            let segment = segment_ptr.segment_mut();
            if segment.can_join(next_segment) && !next_segment.is_allocated() {
                // Since segment_ptr owns next, it's not safe to self.segments.remove(segment_ptr.0)
                // while holding a reference to next. So remove next and coalesce into segment_ptr.
                // This same logic probably applies to the prev case. Need to test.
                segment.range.end = next_segment.range.end;
                self.freelist_remove(next_segment);
                unsafe { self.segments.remove(next.as_ptr()) };
            }
        }
        // If we didn't return, we need to add segment_ptr to a freelist
        self.freelist_insert(segment_ptr);
    }
}

fn ceil_log2(u: usize) -> usize {
    debug_assert!(u != 0);
    (usize::BITS - (u - 1).leading_zeros()) as usize
}

// static PAGE_ALLOCATOR: ResourceAllocator = ResourceAllocator::new_in(Global);
#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec::Vec;

    #[test_case]
    fn make_hashmap() {
        // let random_source = ahash::RandomState::get_src();
        use crate::collections::hash_map::SimpleBuildHasher;
        // page faults because ???
        // let hash_builder: hashbrown::hash_map::DefaultHashBuilder = Default::default();
        let map: HashMap<usize, SegmentPtr<Global>, SimpleBuildHasher> =
            HashMap::with_hasher(Default::default());
    }

    #[test_case]
    fn resource_allocator() {
        let mut ra = ResourceAllocator::<2>::new();
        ra.add(0..10);
        ra.add(20..30);
        let r = ra.fast_allocate(8);
        assert!(r.is_ok());
        ra.release(r.unwrap());
        let r = ra.fast_allocate(10);
        // 2 issues right now
        // - 0..10 seems to have disappeared
        // - 20..28 isn't coalescing properly back with 28..30
        assert!(r.is_ok());
        ra.release(r.unwrap());
        assert!(ra.fast_allocate(20).is_err());
        assert!(ra.fast_allocate(20).is_err());
        let ranges = (0..10)
            .map(|i| {
                ra.fast_allocate(1)
                    .unwrap_or_else(|_| panic!("Failed to allocate block of size 1 ({})", i))
            })
            .collect::<Vec<Range<usize>>>();
        for range in ranges.iter() {
            assert_eq!(range.end - range.start, 2);
        }
        assert!(ra.fast_allocate(1).is_err());
        ranges.into_iter().for_each(|r| ra.release(r));
        let _r1 = ra.fast_allocate(10).unwrap();
        let _r2 = ra.fast_allocate(10).unwrap();
        assert!(ra.fast_allocate(1).is_err());
    }
}
