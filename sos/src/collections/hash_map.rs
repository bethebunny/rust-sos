use alloc::alloc::{Allocator, Global};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};

const PERTURB_SHIFT: usize = 5;
const FIRST_KEY_MASK: usize = 0x1F;
const fn KEY_MASK(size: usize) -> usize {
    size - 1
}

#[derive(Default)]
pub struct SimpleHasher {
    state: u64,
}

impl Hasher for SimpleHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        // TODO: less dumb
        // TODO: vectorize
        for byte in bytes {
            self.state = (self.state << 8) ^ (self.state >> 56) ^ (*byte as u64) * 31
        }
    }
}

pub type SimpleBuildHasher = BuildHasherDefault<SimpleHasher>;

// enum HashMap<K: Eq + Hash, V, A: Allocator + Clone = Global, H: BuildHasher = SimpleBuildHasher> {
//     Empty,
//     Small(Box<SmallHashMap<K, V, A, H, 8>, A>), // hash map size ~= cache line size colocated
//     Large(Box<LargeHashMap<K, V, A, H>, A>),
// }

// trait HashMapImpl<K: Eq + Hash, V> {
//     fn insert(&mut self, k: K, v: V);
//     fn remove(&mut self, k: K);
//     fn contains(&mut self, k: K) -> bool;
// }

// #[derive(Clone, Copy)]
// enum Index<K> {
//     Empty,
//     Deleted,
//     Occupied(K),
// }

// struct Entry<K: Eq + Hash, V> {
//     key_hash: usize,
//     key: K,
//     value: V,
// }

// struct SmallHashMap<K: Eq + Hash, V, A: Allocator + Clone, H: BuildHasher, const S: usize> {
//     allocated: u8,
//     size: u8,
//     indices: [Index<u8>; S], // TODO: parameterize LargeHashMap on this type
//     entries: [Entry<K, V>; S],
//     allocator: A, // Maybe there's a way to not store this since it's on Box?
// }

// struct LargeHashMap<
//     K: Eq + Hash,
//     V,
//     A: Allocator + Clone = Global,
//     H: BuildHasher = SimpleBuildHasher,
// > {
//     allocated: u8, // TODO: parameterize LargeHashMap on this type
//     size: u8,
//     indices: Vec<Index<u8>, A>,
//     entries: Vec<Entry<K, V>, A>,
//     allocator: A,
// }

// impl<K: Eq + Hash, V, A: Allocator + Clone, H: BuildHasher> LargeHashMap<K, V, A> {
//     pub fn with_capacity_in(size: u8, allocator: A) -> Self {
//         let mut indices = Vec::with_capacity_in(size as usize, allocator.clone());
//         indices
//             .spare_capacity_mut()
//             .fill(core::mem::MaybeUninit::new(Index::Empty));
//         // Safety: we just filled these with initialized values
//         unsafe { indices.set_len(size as usize) };
//         LargeHashMap {
//             allocated: 0,
//             size,
//             indices,
//             entries: Vec::with_capacity_in(size as usize, allocator.clone()),
//             allocator,
//         }
//     }

//     #[inline]
//     fn hash(&self, k: &K) -> u64 {
//         let hasher = H::default();
//         k.hash(hasher);
//         hasher.finalize()
//     }

//     #[inline]
//     fn is_key_for_entry(&self, k: &K, key_hash: u64, entry: &Entry<K, V>) -> bool {
//         entry.key_hash == key_hash && k.equals(entry.key)
//     }

//     fn occupied_entry(&self, k: &K) -> Option<&Entry<K, V>> {
//         let hash = self.hash(k);
//         let mask = KEY_MASK(self.size); // Always a power of 2 size
//         let perturb = hash as usize;
//         let i = perturb & mask;
//         loop {
//             match self.indices[i] {
//                 Index::Empty => return None,
//                 Index::Occupied(index) => {
//                     let entry = &self.entries[index];
//                     if entry.key_hash == hash && entry.key.equals(k) {
//                         return Some(entry);
//                     }
//                 }
//                 Index::Deleted => (),
//             }
//             perturb >>= PERTURB_SHIFT;
//             i = mask & (i * 5 + perturb + 1);
//         }
//     }

//     // Whoops TODO what if we need to replace an existing entry
//     fn new_entry_index(&self, k: &K, hash: u64) -> u8 {
//         let mask = KEY_MASK(self.size); // Always a power of 2 size
//         let perturb = hash as usize;
//         let i = perturb & mask;
//         while let Index::Occupied(_) = i {
//             perturb >>= PERTURB_SHIFT;
//             i = mask & (i * 5 + perturb + 1);
//         }
//         i as u8
//     }

//     pub fn contains(&self, k: &K) {
//         self.occupied_entry(k).is_some()
//     }

//     fn should_increase_size(&self) -> bool {
//         self.allocated * 3 >= self.size * 2
//     }

//     fn maybe_resize(&mut self) {
//         todo!();
//     }

//     pub fn insert(&mut self, k: K, v: V) {
//         self.maybe_resize();
//         let hash = self.hash(&k);
//         let index = self.new_index_entry(&k, hash);
//         self.indices[
//         self.entries.append(Entry {
//             key_hash: hash,
//             key: k,
//             value: v,
//         });
//     }
// }

// impl HashMapImpl<K: Eq + Hash, V> for
