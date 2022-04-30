use alloc::alloc::Global;
use alloc::boxed::Box;
use core::alloc::Allocator;
use core::ptr::NonNull;

pub struct DoublyLinkedList<T, A: Allocator + Clone = Global> {
    // TODO: this might need to be Pin, but I don't quite understand
    pub head: Option<Box<DoublyLinkedListNode<T, A>, A>>,
    pub tail: Option<NodePtr<T, A>>,
    allocator: A,
}

impl<T> DoublyLinkedList<T, Global> {
    pub fn new() -> Self {
        DoublyLinkedList {
            head: None,
            tail: None,
            allocator: Global,
        }
    }
}

impl<T, A: Allocator + Clone> DoublyLinkedList<T, A> {
    pub fn new_in(allocator: A) -> DoublyLinkedList<T, A> {
        DoublyLinkedList {
            head: None,
            tail: None,
            allocator,
        }
    }

    fn new_node(&self, value: T) -> Box<DoublyLinkedListNode<T, A>, A> {
        Box::new_in(DoublyLinkedListNode::new(value), self.allocator.clone())
    }

    pub fn append(&mut self, value: T) -> NodePtr<T, A> {
        let mut node = self.new_node(value);
        let node_ptr = node.as_ptr();
        match self.tail {
            Some(mut tail) => {
                let tail = unsafe { tail.as_mut() };
                node.prev = Some(tail.as_ptr());
                tail.next = Some(node);
            }
            None => self.head = Some(node),
        }
        self.tail = Some(node_ptr);
        node_ptr
    }

    pub fn insert_front(&mut self, value: T) -> NodePtr<T, A> {
        let mut node = self.new_node(value);
        let node_ptr = node.as_ptr();
        match self.head {
            Some(ref mut head) => {
                head.prev = Some(node_ptr);
                node.next = self.head.take();
            }
            None => self.tail = Some(node_ptr),
        }
        self.head = Some(node);
        node_ptr
    }

    pub fn pop_front(&mut self) -> Result<T, ()> {
        let head_ptr = self.head.as_mut().ok_or(())?.as_ptr();
        Ok(unsafe { self.remove(head_ptr) })
    }

    // Safety: Caller must guarantee that insert target node is owned by this list
    pub fn insert_after(&mut self, node: &mut NodePtr<T, A>, value: T) -> NodePtr<T, A> {
        let node = unsafe { node.as_mut() };
        let mut new_node = self.new_node(value);
        let new_node_ptr = new_node.as_ptr();
        new_node.next = node.next.take();
        match new_node.next {
            Some(ref mut next) => next.prev = Some(new_node_ptr),
            // this is the tail node
            None => self.tail = Some(new_node.as_ptr()),
        }
        new_node.prev = Some(node.as_ptr());
        node.next = Some(new_node);
        new_node_ptr
    }

    // Safety: Caller must guarantee that removed node is owned by this list
    pub unsafe fn remove(&mut self, mut node: NodePtr<T, A>) -> T {
        let node = node.as_mut();
        // In std::linked all the nodes are linked. Here it's awkward because we need to think
        // about ownership. We need to find the owner of node (either list.head or prev.next),
        // take ownership of the value, and give ownership of node.next.
        let owner = match node.prev {
            Some(mut prev) => &mut prev.as_mut().next,
            None => &mut self.head,
        };
        let owned_node = owner.take().unwrap();
        // update node.next.prev or self.tail
        match node.next {
            Some(ref mut next) => next.prev = node.prev.take(),
            None => self.tail = node.prev, // this is the tail node
        }
        // Give ownership of node.next to the right place
        *owner = node.next.take();
        owned_node.value
    }

    pub fn find<F>(&self, predicate: F) -> Option<NodePtr<T, A>>
    where
        F: Fn(&T) -> bool,
    {
        let mut current = &self.head;
        while let Some(node) = current {
            if predicate(&node.value) {
                return Some(node.as_ptr());
            }
            current = &node.next;
        }
        None
    }

    pub unsafe fn coalesce_left<F>(
        &mut self,
        mut node_ptr: NodePtr<T, A>,
        coalesce: F,
    ) -> NodePtr<T, A>
    where
        F: FnOnce(&T, &T) -> Option<T>,
    {
        let node = node_ptr.as_mut();
        match node.prev {
            Some(mut prev_ptr) => {
                let prev = prev_ptr.as_mut();
                if let Some(new_value) = coalesce(&prev.value, &node.value) {
                    prev.value = new_value;
                    self.remove(node_ptr);
                }
                prev_ptr
            }
            None => node_ptr,
        }
    }

    pub unsafe fn coalesce_right<F>(
        &mut self,
        mut node_ptr: NodePtr<T, A>,
        coalesce: F,
    ) -> NodePtr<T, A>
    where
        F: FnOnce(&T, &T) -> Option<T>,
    {
        let node = node_ptr.as_mut();
        match node.next {
            Some(ref mut next) => {
                if let Some(new_value) = coalesce(&next.value, &node.value) {
                    next.value = new_value;
                    self.remove(node_ptr);
                }
                next.as_ptr()
            }
            None => node_ptr,
        }
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, T, A> {
        Iter {
            next: self.head.as_ref().map(|n| n.as_ptr()),
            marker: core::marker::PhantomData,
        }
    }
}

impl<T: core::fmt::Debug, A: Allocator + Clone> DoublyLinkedList<T, A> {
    pub fn debug_serial_print(&self) {
        crate::serial_println!("DoublyLinkedList[");
        for (i, value) in self.iter().enumerate() {
            crate::serial_println!("  {:<3}: {:#?},", i, value);
        }
        crate::serial_println!("]");
    }
}

pub struct DoublyLinkedListNode<T, A: Allocator = Global> {
    pub next: Option<Box<DoublyLinkedListNode<T, A>, A>>,
    pub prev: Option<NodePtr<T, A>>,
    pub value: T,
}

type NodePtr<T, A> = NonNull<DoublyLinkedListNode<T, A>>;

impl<T, A: Allocator> DoublyLinkedListNode<T, A> {
    pub fn new(value: T) -> Self {
        DoublyLinkedListNode {
            value,
            next: None,
            prev: None,
        }
    }

    pub fn as_ptr(&self) -> NodePtr<T, A> {
        let ptr: *const DoublyLinkedListNode<T, A> = self;
        unsafe { NonNull::new_unchecked(ptr as *mut _) }
    }
}

pub struct Iter<'a, T, A: Allocator + Clone> {
    next: Option<NodePtr<T, A>>,
    marker: core::marker::PhantomData<&'a NodePtr<T, A>>,
}

impl<'a, T, A: Allocator + Clone> Iterator for Iter<'a, T, A> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next.take();
        match next {
            Some(node_ptr) => {
                let node = unsafe { node_ptr.as_ref() };
                self.next = node.next.as_ref().map(|n| n.as_ptr());
                Some(&node.value)
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod test {
    use alloc::alloc::Global;
    use alloc::vec;
    use alloc::vec::Vec;

    use super::*;

    fn verify_integrity<T, A: Allocator + Clone>(ll: &DoublyLinkedList<T, A>, expected: Vec<&T>)
    where
        T: core::fmt::Debug + core::cmp::PartialEq,
    {
        assert_eq!(ll.iter().collect::<Vec<_>>(), expected);
        if expected.is_empty() {
            assert!(ll.head.is_none());
            assert!(ll.tail.is_none());
            return;
        }
        let head = ll.head.as_ref().unwrap();
        let tail = unsafe { ll.tail.unwrap().as_ref() };
        // validate head and tail expectations
        assert_eq!(&head.value, expected[0]);
        assert_eq!(&tail.value, expected[expected.len() - 1]);
        assert!(head.prev.is_none());
        assert!(tail.next.is_none());

        let mut i = 1;
        let mut current = ll.head.as_ref().unwrap();
        while let Some(next) = current.next.as_ref() {
            // validate backward pointers and order
            assert_eq!(
                next.prev
                    .unwrap_or_else(|| panic!("node with value `{:#?}` missing prev", next.value)),
                current.as_ptr()
            );
            assert_eq!(&next.value, expected[i]);
            i += 1;
            current = next;
        }
        assert_eq!(i, expected.len());
    }

    #[test_case]
    fn doubly_linked_list() {
        // let alloc = unsafe { MyAllocator::new() };
        let mut l = DoublyLinkedList::<u8>::new();
        verify_integrity(&l, Vec::<&u8>::new());
        l.append(1);
        verify_integrity(&l, vec![&1u8]);
        l.append(2);
        verify_integrity(&l, vec![&1u8, &2u8]);
        l.append(3);
        verify_integrity(&l, vec![&1u8, &2u8, &3u8]);
        l.pop_front().expect("");
        verify_integrity(&l, vec![&2u8, &3u8]);
        l.pop_front().expect("");
        verify_integrity(&l, vec![&3u8]);
        l.pop_front().expect("");
        verify_integrity(&l, Vec::<&u8>::new());
    }

    #[test_case]
    fn insert_after_and_remove() {
        // Simple test to insert_after and remove some elements and verify integrity along the way.
        let mut l = DoublyLinkedList::<u8>::new();
        verify_integrity(&l, Vec::<&u8>::new());
        let mut one = l.append(1);
        verify_integrity(&l, vec![&1u8]);
        // simple insert_after
        let two = l.insert_after(&mut one, 2);
        verify_integrity(&l, vec![&1u8, &2u8]);
        // interior insert
        let mut three = l.insert_after(&mut one, 3);
        verify_integrity(&l, vec![&1u8, &3u8, &2u8]);
        // remove tail
        unsafe { l.remove(two) };
        verify_integrity(&l, vec![&1u8, &3u8]);
        // insert_after on tail
        l.insert_after(&mut three, 4);
        verify_integrity(&l, vec![&1u8, &3u8, &4u8]);
        // remove interior
        unsafe { l.remove(three) };
        verify_integrity(&l, vec![&1u8, &4u8]);
    }

    // #[test_case]
    // fn remove_drops() {
    //     // Create very small stack memory space, enough for a few nodes
    //     // Create a BumpAllocator for it
    //     // ~5x, insert a few list nodes and remove them
    // }

    #[test_case]
    fn doubly_linked_list_node_size() {
        // For a ZST value + ZST allocator, the Node size should be exactly 2 pointers
        assert_eq!(
            core::mem::size_of::<DoublyLinkedListNode<(), Global>>(),
            2 * core::mem::size_of::<usize>(),
        )
    }
}
