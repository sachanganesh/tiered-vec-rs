use std::{
    alloc::{alloc_zeroed, realloc, Layout, LayoutError},
    fmt::Debug,
    marker::PhantomData,
    mem::{size_of, MaybeUninit},
    ops::{Index, IndexMut},
    ptr::{self, NonNull},
};

use crate::error::{TierError, TieredVectorError};

use super::tier::Tier;

pub struct FlatTieredVec<T>
where
    T: Debug,
{
    ptr: *mut u8,
    tier_capacity: usize,
    len: usize,
    marker: PhantomData<T>,
}

impl<T> FlatTieredVec<T>
where
    T: Debug,
{
    pub fn new(tier_capacity: usize) -> Self {
        assert!(tier_capacity.is_power_of_two());
        assert!(tier_capacity.ge(&2));

        let layout =
            Self::layout_for(tier_capacity).expect("memory layout for tier size should be valid");

        let buffer_ptr = unsafe { alloc_zeroed(layout) };

        Self {
            ptr: buffer_ptr,
            tier_capacity,
            len: 0,
            marker: PhantomData,
        }
    }

    pub fn with_capacity(minimum_capacity: usize) -> Self {
        assert!(minimum_capacity.ge(&4));

        let mut capacity = minimum_capacity;
        if !capacity.is_power_of_two() {
            capacity = capacity.next_power_of_two();
        }

        let trailing = capacity.trailing_zeros();
        let shift_count = if trailing & 1 == 0 {
            trailing / 2
        } else {
            capacity = capacity << 1;
            (trailing + 1) / 2
        };

        let tier_capacity = capacity >> shift_count;
        Self::new(tier_capacity)
    }

    fn layout_from(tier_layout: Layout, tier_capacity: usize) -> Result<Layout, LayoutError> {
        Layout::from_size_align(tier_layout.size() * tier_capacity, tier_layout.align())
    }

    fn layout_for(tier_capacity: usize) -> Result<Layout, LayoutError> {
        let tier_layout = Tier::<T>::layout_for(tier_capacity)?;
        Self::layout_from(tier_layout, tier_capacity)
    }

    #[inline]
    fn size_of_tier(capacity: usize) -> usize {
        Tier::<T>::layout_for(capacity)
            .expect("memory layout for tier size should be valid")
            .size()
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.tier_capacity.pow(2)
    }

    #[inline]
    const fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    const fn num_tiers(&self) -> usize {
        self.tier_capacity
    }

    #[inline]
    pub const fn tier_capacity(&self) -> usize {
        self.tier_capacity
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    const fn tier_index(&self, rank: usize) -> usize {
        rank / self.tier_capacity()
    }

    fn raw_tier_ptr_from_capacity(&self, index: usize, tier_capacity: usize) -> *mut Tier<T> {
        assert!(index < tier_capacity);

        unsafe {
            ptr::slice_from_raw_parts_mut(
                self.ptr.add(index * Self::size_of_tier(tier_capacity)),
                tier_capacity,
            ) as _
        }
    }

    fn raw_tier_ptr(&self, index: usize) -> *mut Tier<T> {
        self.raw_tier_ptr_from_capacity(index, self.tier_capacity())
    }

    pub(crate) fn tier(&self, index: usize) -> &Tier<T> {
        let tier = unsafe { &*self.raw_tier_ptr(index) };
        assert_eq!(self.tier_capacity(), tier.elements.len());

        return tier;
    }

    pub(crate) fn tier_mut(&mut self, index: usize) -> &mut Tier<T> {
        let tier = unsafe { &mut *self.raw_tier_ptr(index) };
        assert_eq!(self.tier_capacity(), tier.elements.len());

        return tier;
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tier<T>> {
        (0..self.num_tiers()).map(move |i| unsafe { &*self.raw_tier_ptr(i) })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Tier<T>> {
        (0..self.num_tiers()).map(move |i| unsafe { &mut *self.raw_tier_ptr(i) })
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.tier(self.tier_index(index)).get_by_rank(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.tier_mut(self.tier_index(index)).get_by_rank_mut(index)
    }

    fn expand(&mut self) {
        let curr_tier_capacity = self.tier_capacity();
        let new_tier_capacity = self.tier_capacity() << 1;

        let curr_tier_layout = Tier::<T>::layout_for(curr_tier_capacity)
            .expect("memory layout for current tier size should be valid");
        let curr_layout = Self::layout_from(curr_tier_layout, curr_tier_capacity)
            .expect("memory layout for current tier size should be valid");

        let new_tier_layout = Tier::<T>::layout_for(new_tier_capacity)
            .expect("memory layout for current tier size should be valid");
        let new_layout = Self::layout_from(new_tier_layout, new_tier_capacity)
            .expect("memory layout for current tier size should be valid");

        // loop through every consecutive pair of tiers and merge them
        for i in (0..curr_tier_capacity).step_by(2) {
            let first_tier_ptr = self.raw_tier_ptr(i);
            let second_tier_ptr = self.raw_tier_ptr(i + 1);

            // ensure that tiers are in sorted linear order
            let first_tier = unsafe { &mut *first_tier_ptr };
            first_tier.rotate_reset();

            let second_tier = unsafe { &mut *second_tier_ptr };
            second_tier.rotate_reset();

            // keep track of this value now as it will be overwritten in the next step
            let second_tier_len = second_tier.len();

            // copy second tier's elements such that it extends first tier's slice
            unsafe {
                // read from start of second tier's element list
                let read_ptr = second_tier.elements.as_mut_ptr();

                // write to end of first tier's element list
                let write_ptr = first_tier.elements.as_mut_ptr().add(first_tier.len());

                ptr::copy(read_ptr, write_ptr, second_tier_len);
            }

            // ensure first tier now tracks the newly merged elements
            first_tier.tail_forward_by(second_tier_len);

            // copy the merged tier to the start of where newly sized tier sits in memory
            let read_ptr = first_tier_ptr as *mut u8;
            let write_ptr = self.raw_tier_ptr_from_capacity(i / 2, new_tier_capacity) as *mut u8;

            unsafe {
                ptr::copy(read_ptr, write_ptr, new_tier_layout.size());
            }
        }

        // reallocate and assign new tier_capacity
        self.ptr = unsafe { realloc(self.ptr, curr_layout, new_layout.size()) };
        self.tier_capacity = new_tier_capacity;

        // remaining are new tiers to be cleared out
        for i in (curr_tier_capacity / 2)..new_tier_capacity {
            self.tier_mut(i).clear_and_leak();
        }
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        assert!(index <= self.len());

        if self.is_full() {
            self.expand();
        }

        let tier_index = self.tier_index(index);
        let mut prev_popped = None;

        for i in tier_index..self.num_tiers() {
            let tier = self.tier_mut(i);

            if tier.is_full() {
                let popped = tier.pop_back();

                if let Some(prev_elem) = prev_popped {
                    tier.push_front(prev_elem);
                }

                prev_popped = Some(popped);
            } else {
                if let Some(prev_elem) = prev_popped.take() {
                    tier.push_front(prev_elem);
                }

                break;
            }
        }

        self.tier_mut(tier_index).insert(index, elem);
        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> T {
        let num_entries = self.len();
        assert!(index < num_entries);

        let tier_index = self.tier_index(index);
        let mut prev_popped = None;

        // shift phase
        let elem = self.tier_mut(tier_index).remove(index);
        self.len -= 1;

        // pop-push phase
        let last_tier_index = self.tier_index(num_entries);
        for i in (tier_index + 1..last_tier_index + 1).rev() {
            let tier = self.tier_mut(i);

            if !tier.is_empty() {
                let popped = tier.pop_front();
                if let Some(prev_elem) = prev_popped {
                    tier.push_back(prev_elem);
                }

                prev_popped = Some(popped);
            }
        }

        if let Some(popped) = prev_popped {
            self.tier_mut(tier_index).push_back(popped);
        }

        return elem;
    }

    pub fn push(&mut self, elem: T) {
        if self.is_full() {
            self.expand();
        }

        let tier = self.tier_mut(self.tier_index(self.len()));
        assert!(!tier.is_full());

        tier.push_back(elem);
        self.len += 1;
    }

    pub fn pop(&mut self) -> T {
        assert!(!self.is_empty());

        let tier = self.tier_mut(self.tier_index(self.len() - 1));
        assert!(!tier.is_empty());

        let elem = tier.pop_back();

        self.len -= 1;
        return elem;
    }

    // fn try_contract(&mut self, num_entries: usize) {
    //     // only contract well below capacity to cull repeated alloc/free of memory upon reinsertion/redeletion
    //     if num_entries < self.capacity() / 8 {
    //         let curr_tier_capacity = self.tier_capacity();
    //         let new_tier_capacity = curr_tier_capacity >> 1;

    //         let split_index = new_tier_capacity >> 1;
    //         let _ = self.offsets.split_off(split_index);

    //         let end_index = new_tier_capacity;
    //         for i in (0..end_index).step_by(2) {
    //             let old_start_index = i * curr_tier_capacity;
    //             let old_end_index = old_start_index + curr_tier_capacity;
    //             let old_tier = &mut self.buffer[old_start_index..old_end_index];
    //             let old_ring_offsets = &mut self.offsets[i];

    //             let new_ring_offsets = ImplicitTier::split_half(old_tier, old_ring_offsets);
    //             self.offsets.insert(i + 1, new_ring_offsets);
    //         }

    //         let _ = self.buffer.split_off(split_index * curr_tier_capacity);

    //         assert_eq!(self.offsets.len(), new_tier_capacity);
    //     }
    // }
}

impl<T> Index<usize> for FlatTieredVec<T>
where
    T: Debug,
{
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len());
        &self.tier(self.tier_index(index))[index]
    }
}

impl<T> IndexMut<usize> for FlatTieredVec<T>
where
    T: Debug,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len());
        &mut self.tier_mut(self.tier_index(index))[index]
    }
}

impl<T> Clone for FlatTieredVec<T>
where
    T: Clone + Debug,
{
    fn clone(&self) -> Self {
        let layout = Self::layout_for(self.tier_capacity())
            .expect("memory layout for tier size should be valid");

        let buffer_ptr = unsafe { alloc_zeroed(layout) };

        let mut cloned = Self {
            ptr: buffer_ptr,
            tier_capacity: self.tier_capacity(),
            len: 0,
            marker: PhantomData,
        };

        for i in 0..self.len() {
            cloned.push(self[i].clone());
        }

        return cloned;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn error_on_non_power_of_two_size() {
        let _t: FlatTieredVec<usize> = FlatTieredVec::new(5);
    }

    #[test]
    #[should_panic]
    fn error_on_small_size() {
        let _t: FlatTieredVec<usize> = FlatTieredVec::new(1);
    }

    #[test]
    fn no_error_on_correct_size() {
        let size = 4;
        let t: FlatTieredVec<usize> = FlatTieredVec::new(size);
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), size * size);
        assert_eq!(t.tier_capacity(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn with_capacity() {
        let mut t: FlatTieredVec<usize> = FlatTieredVec::with_capacity(4);
        assert_eq!(4, t.capacity());
        assert_eq!(2, t.tier_capacity());

        t = FlatTieredVec::with_capacity(8);
        assert_eq!(16, t.capacity());
        assert_eq!(4, t.tier_capacity());

        t = FlatTieredVec::with_capacity(128);
        assert_eq!(256, t.capacity());
        assert_eq!(16, t.tier_capacity());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: FlatTieredVec<usize> = FlatTieredVec::new(size);
        assert_eq!(t.tier_capacity(), size);

        for i in 0..size {
            t.insert(i, i * 2);
            assert_eq!(t.len(), i + 1);
        }

        for i in 0..size {
            let result = t.get(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i * 2);
        }

        assert_eq!(t.len(), size);
        assert!(!t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn expand() {
        let size = 4;
        let mut t: FlatTieredVec<usize> = FlatTieredVec::new(size);

        for i in 0..size * size {
            t.insert(i, i);
            assert_eq!(*t.get(i).unwrap(), i);
        }
        assert_eq!(t.tier_capacity(), size);
        assert_eq!(t.len(), size * size);
        assert!(t.is_full());

        t.insert(size * size, size * size);
        assert_eq!(t.tier_capacity(), size * 2);
        assert_eq!(t.len(), (size * size) + 1);
        assert!(!t.is_full());

        for i in 0..((size * size) + 1) {
            let result = t.get(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i);
        }
    }

    // #[test]
    // fn remove_and_contract() {
    //     let size = 16;
    //     let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);
    //     assert_eq!(t.capacity(), size * size);

    //     for i in 0..size * size / 8 {
    //         // size / 8 {
    //         t.insert(i, i).is_ok();
    //         assert_eq!(*t.get(i).unwrap(), i);
    //     }
    //     assert_eq!(t.tier_capacity(), size);
    //     assert_eq!(t.len(), size * size / 8);
    //     assert_eq!(t.capacity(), size * size);

    //     assert!(t.remove(0).is_ok());

    //     assert_eq!(*t.get(0).unwrap(), 1);
    //     assert_eq!(t.len(), (size * size / 8) - 1);
    //     assert_eq!(t.capacity(), size * size);

    //     // contract
    //     assert!(t.remove(0).is_ok());

    //     assert_eq!(*t.get(0).unwrap(), 2);
    //     assert_eq!(t.len(), (size * size / 8) - 2);
    //     assert_eq!(t.capacity(), size * size / 4);
    // }
}
