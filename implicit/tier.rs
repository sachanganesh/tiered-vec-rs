use std::{
    mem::{ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
};

use crate::cache_conscious::tiered_vec::tier::TierError;

use super::tier_ring_offsets::ImplicitTierRingOffsets;

#[repr(transparent)]
pub struct ImplicitTier<'a, T>(&'a mut Box<[MaybeUninit<T>]>)
where
    T: Copy;

impl<'a, T> Deref for ImplicitTier<'a, T>
where
    T: Copy,
{
    type Target = Box<[MaybeUninit<T>]>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, T> DerefMut for ImplicitTier<'a, T>
where
    T: Copy,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

impl<'a, T> ImplicitTier<'a, T>
where
    T: Copy,
{
    pub fn from_slice(slice: &'a mut Box<[MaybeUninit<T>]>) -> Self {
        assert!(slice.len().is_power_of_two());
        Self(slice)
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.0.len()
    }

    #[inline]
    const fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    const fn masked_index_is_unused(
        &self,
        ring_offsets: &ImplicitTierRingOffsets,
        masked_idx: usize,
    ) -> bool {
        let masked_head = self.mask(ring_offsets.head());
        let masked_tail = self.mask(ring_offsets.tail());

        if masked_head < masked_tail {
            masked_idx < masked_head || (masked_idx >= masked_tail && masked_idx < self.capacity())
        } else {
            masked_idx >= masked_tail && masked_idx < masked_head
        }
    }

    #[inline]
    pub(crate) const fn is_valid_masked_index(
        &self,
        ring_offsets: &ImplicitTierRingOffsets,
        masked_idx: usize,
    ) -> bool {
        !self.masked_index_is_unused(ring_offsets, masked_idx)
    }

    const fn contains_masked_rank(
        &self,
        ring_offsets: &ImplicitTierRingOffsets,
        masked_rank: usize,
    ) -> bool {
        !self.masked_index_is_unused(ring_offsets, masked_rank)
    }

    pub const fn contains_rank(&self, ring_offsets: &ImplicitTierRingOffsets, rank: usize) -> bool {
        self.contains_masked_rank(
            ring_offsets,
            self.mask(ring_offsets.head().wrapping_add(rank)),
        )
    }

    pub fn get(&self, ring_offsets: &ImplicitTierRingOffsets, idx: usize) -> Option<&T> {
        let masked_idx = self.mask(idx);
        if !self.is_valid_masked_index(ring_offsets, masked_idx) {
            return None;
        }

        let elem = &self.0[masked_idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub fn get_mut(
        &mut self,
        ring_offsets: &ImplicitTierRingOffsets,
        idx: usize,
    ) -> Option<&mut T> {
        let masked_idx = self.mask(idx);
        if !self.is_valid_masked_index(ring_offsets, masked_idx) {
            return None;
        }

        let elem = &mut self.0[masked_idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank(&self, ring_offsets: &ImplicitTierRingOffsets, rank: usize) -> Option<&T> {
        self.get(ring_offsets, ring_offsets.head().wrapping_add(rank))
    }

    pub fn get_mut_by_rank(
        &mut self,
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> Option<&mut T> {
        self.get_mut(ring_offsets, ring_offsets.head().wrapping_add(rank))
    }

    fn set(&mut self, masked_idx: usize, elem: T) -> &mut T {
        self.0[masked_idx].write(elem)
    }

    fn take(&mut self, masked_idx: usize) -> T {
        let slot = &mut self.0[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::uninit()).assume_init() }
    }

    fn replace(&mut self, masked_idx: usize, elem: T) -> T {
        let slot = &mut self.0[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::new(elem)).assume_init() }
    }

    pub fn push_front(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        if !ring_offsets.is_full(self.capacity()) {
            ring_offsets.head_backward();
            let idx = self.mask(ring_offsets.head());

            self.set(idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn push_back(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        if !ring_offsets.is_full(self.capacity()) {
            let idx = self.mask(ring_offsets.tail());
            ring_offsets.tail_forward();

            self.set(idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn pop_front(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
    ) -> Result<T, TierError<T>> {
        if !ring_offsets.is_empty() {
            let idx = self.mask(ring_offsets.head());
            ring_offsets.head_forward();

            Ok(self.take(idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    pub fn pop_back(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
    ) -> Result<T, TierError<T>> {
        if !ring_offsets.is_empty() {
            ring_offsets.tail_backward();
            let idx = self.mask(ring_offsets.tail());

            Ok(self.take(idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    fn shift_to_head(&mut self, ring_offsets: &mut ImplicitTierRingOffsets, from: usize) {
        let mut cursor: Option<T> = None;
        let mut i = from;

        ring_offsets.head_backward();

        while i != self.mask(ring_offsets.head()) {
            if let Some(curr_elem) = cursor {
                let elem = self.replace(i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = self.take(i);
                cursor = Some(elem);
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(curr_elem) = cursor {
            self.set(i, curr_elem);
        }
    }

    fn shift_to_tail(&mut self, ring_offsets: &mut ImplicitTierRingOffsets, from: usize) {
        let masked_tail = self.mask(ring_offsets.tail());
        let mut cursor: Option<T> = None;
        let mut i = from;

        while i < masked_tail {
            if let Some(curr_elem) = cursor {
                cursor = Some(self.replace(i, curr_elem));
            } else {
                cursor = Some(self.take(i));
            }

            i = self.mask(i.wrapping_add(1));
        }

        if let Some(curr_elem) = cursor {
            self.set(i, curr_elem);
            ring_offsets.tail_forward();
        }
    }

    pub fn insert_at_rank(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        let masked_head = self.mask(ring_offsets.head());
        let masked_tail = self.mask(ring_offsets.tail());
        let masked_rank = self.mask(ring_offsets.head().wrapping_add(rank));

        // todo: investigate case in which tier is empty but rank > 0 needs to be inserted
        if !self.contains_masked_rank(ring_offsets, masked_rank) {
            // if no element at rank, insert
            if masked_head == masked_rank {
                self.push_front(ring_offsets, elem)
            } else if masked_tail == masked_rank {
                self.push_back(ring_offsets, elem)
            } else {
                Err(TierError::TierDisconnectedEntryInsertionError(rank, elem))
            }
        } else {
            // conversion should not fail given normalized values
            // unless tier size is so large that it overflows isize

            let head_delta = masked_rank.abs_diff(masked_head);
            let tail_delta = masked_rank.abs_diff(masked_tail);

            if head_delta <= tail_delta {
                self.shift_to_head(ring_offsets, masked_rank);
            } else {
                self.shift_to_tail(ring_offsets, masked_rank);
            }

            self.set(masked_rank, elem);
            Ok(masked_rank)
        }
    }

    fn close_gap(&mut self, ring_offsets: &mut ImplicitTierRingOffsets, gap_masked_idx: usize) {
        let mut cursor = None;

        ring_offsets.tail_backward();
        let mut i: usize = self.mask(ring_offsets.tail());

        while i > gap_masked_idx {
            if let Some(elem) = cursor {
                cursor = Some(self.replace(i, elem));
            } else {
                cursor = Some(self.take(i));
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(elem) = cursor {
            self.set(i, elem);
        }
    }

    pub fn remove_at_rank(
        &mut self,
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
    ) -> Result<T, TierError<T>> {
        let masked_rank = self.mask(ring_offsets.head().wrapping_add(rank));

        if self.contains_masked_rank(ring_offsets, masked_rank) {
            let elem = self.take(masked_rank);

            if masked_rank == self.mask(ring_offsets.head()) {
                ring_offsets.head_forward();
            } else if masked_rank == self.mask(ring_offsets.tail()) {
                ring_offsets.tail_backward();
            } else {
                self.close_gap(ring_offsets, masked_rank);
            }

            Ok(elem)
        } else {
            Err(TierError::TierIndexOutOfBoundsError(rank))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cache_conscious::tiered_vec::implicit::tier::*;

    fn prepare_slice(len: usize) -> Box<[MaybeUninit<usize>]> {
        let mut v = Vec::with_capacity(len);
        unsafe {
            v.set_len(len);
        }

        v.into_boxed_slice()
    }

    fn prepare_tier<T: Copy>(slice: &mut Box<[MaybeUninit<T>]>) -> ImplicitTier<T> {
        ImplicitTier::from_slice(slice)
    }

    fn prepare_ring_offsets() -> ImplicitTierRingOffsets {
        ImplicitTierRingOffsets::default()
    }

    #[test]
    #[should_panic]
    fn error_on_wrong_tier_size() {
        let mut s = prepare_slice(3);
        let _t: ImplicitTier<usize> = prepare_tier(&mut s);
    }

    #[test]
    fn no_error_on_correct_tier_size() {
        let mut s = prepare_slice(4);
        let t: ImplicitTier<usize> = prepare_tier(&mut s);
        assert_eq!(t.capacity(), 4);
    }

    #[test]
    fn insert_at_rank_shift_head() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());

        // [1, 3, 2, 0]
        assert!(t.insert_at_rank(&mut ring_offsets, 1, 3).is_ok());
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 3);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 0);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 3);
    }

    #[test]
    fn insert_at_rank_shift_tail() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());

        // [0, 1, 3, 2]
        assert!(t.insert_at_rank(&mut ring_offsets, 2, 3).is_ok());
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 0);
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 3);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn remove_at_rank_1() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());
        assert!(t.push_back(&mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.masked_head(t.capacity()), 0);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 0);

        // [0, 2, 3, _]
        assert!(t.remove_at_rank(&mut ring_offsets, 1).is_ok());
        assert_eq!(ring_offsets.masked_head(t.capacity()), 0);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 3);
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 0);
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 2);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 3);
        assert!(t.get(&ring_offsets, 3).is_none());
    }

    #[test]
    fn remove_at_rank_2() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());
        assert!(t.push_back(&mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.masked_head(t.capacity()), 0);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 0);

        // [_, 1, 2, 3]
        assert!(t.remove_at_rank(&mut ring_offsets, 0).is_ok());
        assert_eq!(ring_offsets.masked_head(t.capacity()), 1);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 0);
        assert!(t.get(&ring_offsets, 0).is_none());
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 3);
    }
    #[test]
    fn shift_to_head_basic() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());

        // [1, 2, n, 0]
        t.shift_to_head(&mut ring_offsets, 2);
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 2);
        assert_ne!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 0);
    }

    #[test]
    fn shift_to_head_data_middle_1() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [n, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());
        assert!(t.pop_front(&mut ring_offsets).is_ok());

        // [1, n, 2, n]
        t.shift_to_head(&mut ring_offsets, 1);
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 1);
        assert_ne!(*t.get(&ring_offsets, 1).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert!(t.get(&ring_offsets, 3).is_none());
    }

    #[test]
    fn shift_to_head_data_middle_2() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [n, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());
        assert!(t.pop_front(&mut ring_offsets).is_ok());

        // [1, 2, n, n]
        t.shift_to_head(&mut ring_offsets, 1);
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert_ne!(*t.get(&ring_offsets, 1).unwrap(), 2);
        assert!(t.get(&ring_offsets, 3).is_none());
    }

    #[test]
    fn shift_to_tail_nonwrapping() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());

        // [0, n, 1, 2]
        t.shift_to_tail(&mut ring_offsets, 1);
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 0);
        assert_ne!(*t.get(&ring_offsets, 1).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn shift_to_tail_wrapping() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, n, 1, 2]
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert!(t.push_back(&mut ring_offsets, 2).is_ok());
        assert!(t.pop_front(&mut ring_offsets).is_ok());
        assert!(t.pop_front(&mut ring_offsets).is_ok());
        assert!(t.push_back(&mut ring_offsets, 0).is_ok());

        // [n, 0, 1, 2]
        t.shift_to_tail(&mut ring_offsets, 0);
        assert_ne!(*t.get(&ring_offsets, 0).unwrap(), 0);
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 0);
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 1);
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn push_and_pop() {
        let mut s = prepare_slice(4);
        let mut t: ImplicitTier<usize> = prepare_tier(&mut s);
        let mut ring_offsets = prepare_ring_offsets();

        assert!(ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(t.capacity()));
        assert_eq!(ring_offsets.len(), 0);
        assert_eq!(t.capacity(), 4);

        // [n, n, n, 0]
        assert!(t.push_front(&mut ring_offsets, 0).is_ok());
        assert_eq!(ring_offsets.len(), 1);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 3);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 0);
        assert!(t.get(&ring_offsets, 3).is_some());
        assert_eq!(*t.get(&ring_offsets, 3).unwrap(), 0);
        assert_eq!(*t.get_by_rank(&ring_offsets, 0).unwrap(), 0);

        assert!(!t.is_valid_masked_index(&ring_offsets, 0));
        assert!(!t.is_valid_masked_index(&ring_offsets, 1));
        assert!(!t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        // [1, n, n, 0]
        assert!(t.push_back(&mut ring_offsets, 1).is_ok());
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 3);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 1);
        assert!(t.get(&ring_offsets, 0).is_some());
        assert_eq!(*t.get(&ring_offsets, 0).unwrap(), 1);
        assert_eq!(*t.get_by_rank(&ring_offsets, 1).unwrap(), 1);

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(!t.is_valid_masked_index(&ring_offsets, 1));
        assert!(!t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        // [1, n, 2, 0]
        assert!(t.push_front(&mut ring_offsets, 2).is_ok());
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 2);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 1);
        assert!(t.get(&ring_offsets, 2).is_some());
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 2);
        assert_eq!(*t.get_by_rank(&ring_offsets, 0).unwrap(), 2);

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(!t.is_valid_masked_index(&ring_offsets, 1));
        assert!(t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        // [1, 3, 2, 0]
        assert!(t.push_back(&mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.len(), 4);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 2);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 2);
        assert!(t.get(&ring_offsets, 1).is_some());
        assert_eq!(*t.get(&ring_offsets, 1).unwrap(), 3);
        assert_eq!(*t.get_by_rank(&ring_offsets, 3).unwrap(), 3);

        assert!(!ring_offsets.is_empty());
        assert!(ring_offsets.is_full(t.capacity()));

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(t.is_valid_masked_index(&ring_offsets, 1));
        assert!(t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        assert!(t.push_back(&mut ring_offsets, 4).is_err());
        assert_eq!(ring_offsets.masked_head(t.capacity()), 2);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 2);

        // [1, 3, n, 0]
        let mut v = t.pop_front(&mut ring_offsets);
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 2);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(t.capacity()));
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 3);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 2);
        assert!(t.get(&ring_offsets, 2).is_none());

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(t.is_valid_masked_index(&ring_offsets, 1));
        assert!(!t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        // [1, n, n, 0]
        v = t.pop_back(&mut ring_offsets);
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 3);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(t.capacity()));
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 3);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 1);
        assert!(t.get(&ring_offsets, 1).is_none());

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(!t.is_valid_masked_index(&ring_offsets, 1));
        assert!(!t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));

        // [1, n, 4, 0]
        assert!(t.push_front(&mut ring_offsets, 4).is_ok());
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(t.capacity()), 2);
        assert_eq!(ring_offsets.masked_tail(t.capacity()), 1);
        assert!(t.get(&ring_offsets, 2).is_some());
        assert_eq!(*t.get(&ring_offsets, 2).unwrap(), 4);
        assert_eq!(*t.get_by_rank(&ring_offsets, 0).unwrap(), 4);

        assert!(t.is_valid_masked_index(&ring_offsets, 0));
        assert!(!t.is_valid_masked_index(&ring_offsets, 1));
        assert!(t.is_valid_masked_index(&ring_offsets, 2));
        assert!(t.is_valid_masked_index(&ring_offsets, 3));
    }
}
