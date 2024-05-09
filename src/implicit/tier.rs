use std::{fmt::Debug, marker::PhantomData, mem::MaybeUninit};

use crate::tier_error::TierError;

use super::tier_ring_offsets::ImplicitTierRingOffsets;

pub struct ImplicitTier<T>
where
    T: Clone + Debug,
{
    marker: PhantomData<T>,
}

impl<T> ImplicitTier<T>
where
    T: Clone + Debug,
{
    #[inline(always)]
    pub const fn capacity(tier: &[MaybeUninit<T>]) -> usize {
        tier.len()
    }

    #[inline(always)]
    const fn mask(tier: &[MaybeUninit<T>], val: usize) -> usize {
        val & (Self::capacity(tier) - 1)
    }

    #[inline]
    const fn masked_index_is_unused(
        tier: &[MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        masked_idx: usize,
    ) -> bool {
        let masked_head = Self::mask(tier, ring_offsets.head());
        let masked_tail = Self::mask(tier, ring_offsets.tail());

        if masked_head < masked_tail {
            masked_idx < masked_head
                || (masked_idx >= masked_tail && masked_idx < Self::capacity(tier))
        } else {
            masked_idx >= masked_tail && masked_idx < masked_head
        }
    }

    #[inline]
    pub(crate) const fn is_valid_masked_index(
        tier: &[MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        masked_idx: usize,
    ) -> bool {
        !Self::masked_index_is_unused(tier, ring_offsets, masked_idx)
    }

    const fn contains_masked_rank(
        tier: &[MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        masked_rank: usize,
    ) -> bool {
        let masked_head = ring_offsets.masked_head(Self::capacity(tier));
        let masked_tail = ring_offsets.masked_tail(Self::capacity(tier));

        if ring_offsets.is_full(Self::capacity(tier)) {
            true
        } else if masked_head <= masked_tail {
            // standard case
            masked_rank >= masked_head && masked_rank < masked_tail
        } else {
            // wrapping case
            masked_rank >= masked_head || masked_rank < masked_tail
        }
    }

    pub const fn contains_rank(
        tier: &[MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> bool {
        Self::contains_masked_rank(
            tier,
            ring_offsets,
            ring_offsets.masked_rank(rank, Self::capacity(tier)),
        )
    }

    pub fn get<'a>(
        tier: &'a [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        idx: usize,
    ) -> Option<&'a T> {
        let masked_idx = Self::mask(tier, idx);
        if !Self::is_valid_masked_index(tier, ring_offsets, masked_idx) {
            return None;
        }

        let elem = &tier[masked_idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub fn get_mut<'a>(
        tier: &'a mut [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        idx: usize,
    ) -> Option<&'a mut T> {
        let masked_idx = Self::mask(tier, idx);
        if !Self::is_valid_masked_index(tier, ring_offsets, masked_idx) {
            return None;
        }

        let elem = &mut tier[masked_idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank<'a>(
        tier: &'a [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> Option<&'a T> {
        Self::get(tier, ring_offsets, ring_offsets.head().wrapping_add(rank))
    }

    pub fn get_mut_by_rank<'a>(
        tier: &'a mut [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> Option<&'a mut T> {
        Self::get_mut(tier, ring_offsets, ring_offsets.head().wrapping_add(rank))
    }

    fn set(tier: &mut [MaybeUninit<T>], masked_idx: usize, elem: T) -> &mut T {
        tier[masked_idx].write(elem)
    }

    fn take(tier: &mut [MaybeUninit<T>], masked_idx: usize) -> T {
        let slot = &mut tier[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::zeroed()).assume_init() }
    }

    fn replace(tier: &mut [MaybeUninit<T>], masked_idx: usize, elem: T) -> T {
        let slot = &mut tier[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::new(elem)).assume_init() }
    }

    pub fn push_front(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        let cap = Self::capacity(tier);
        if !ring_offsets.is_full(cap) {
            ring_offsets.head_backward();
            let idx = ring_offsets.masked_head(cap);

            Self::set(tier, idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn push_back(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        let cap = Self::capacity(tier);
        if !ring_offsets.is_full(cap) {
            let idx = ring_offsets.masked_tail(cap);
            ring_offsets.tail_forward();

            Self::set(tier, idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn pop_front(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
    ) -> Result<T, TierError<T>> {
        if !ring_offsets.is_empty() {
            let idx = ring_offsets.masked_head(Self::capacity(tier));
            ring_offsets.head_forward();

            Ok(Self::take(tier, idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    pub fn pop_back(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
    ) -> Result<T, TierError<T>> {
        if !ring_offsets.is_empty() {
            ring_offsets.tail_backward();
            let idx = ring_offsets.masked_tail(Self::capacity(tier));

            Ok(Self::take(tier, idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    fn shift_to_head(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        from: usize,
    ) {
        let mut cursor: Option<T> = None;
        let mut i = from;

        ring_offsets.head_backward();
        let masked_head = ring_offsets.masked_head(Self::capacity(tier));

        while i != masked_head {
            if let Some(curr_elem) = cursor {
                let elem = Self::replace(tier, i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = Self::take(tier, i);
                cursor = Some(elem);
            }

            i = Self::mask(tier, i.wrapping_sub(1));
        }

        if let Some(curr_elem) = cursor {
            Self::set(tier, i, curr_elem);
        }
    }

    fn shift_to_tail(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        from: usize,
    ) {
        let masked_tail = ring_offsets.masked_tail(Self::capacity(tier));
        let mut cursor: Option<T> = None;
        let mut i = from;

        while i < masked_tail {
            if let Some(curr_elem) = cursor {
                cursor = Some(Self::replace(tier, i, curr_elem));
            } else {
                cursor = Some(Self::take(tier, i));
            }

            i = Self::mask(tier, i.wrapping_add(1));
        }

        if let Some(curr_elem) = cursor {
            Self::set(tier, i, curr_elem);
            ring_offsets.tail_forward();
        }
    }

    pub fn insert_at_rank(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
        elem: T,
    ) -> Result<usize, TierError<T>> {
        let masked_head = Self::mask(tier, ring_offsets.head());
        let masked_tail = Self::mask(tier, ring_offsets.tail());
        let masked_rank = Self::mask(tier, ring_offsets.head().wrapping_add(rank));

        // todo: investigate case in which tier is empty but rank > 0 needs to be inserted
        if !Self::contains_masked_rank(tier, ring_offsets, masked_rank) {
            // if no element at rank, insert
            if masked_head == masked_rank {
                Self::push_front(tier, ring_offsets, elem)
            } else if masked_tail == masked_rank {
                Self::push_back(tier, ring_offsets, elem)
            } else {
                Err(TierError::TierDisconnectedEntryInsertionError(rank, elem))
            }
        } else {
            // conversion should not fail given normalized values
            // unless tier size is so large that it overflows isize

            let head_delta = masked_rank.abs_diff(masked_head);
            let tail_delta = masked_rank.abs_diff(masked_tail);

            if head_delta <= tail_delta {
                Self::shift_to_head(tier, ring_offsets, masked_rank);
            } else {
                Self::shift_to_tail(tier, ring_offsets, masked_rank);
            }

            Self::set(tier, masked_rank, elem);
            Ok(masked_rank)
        }
    }

    fn close_gap(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        gap_masked_idx: usize,
    ) {
        let mut cursor = None;

        ring_offsets.tail_backward();
        let mut i: usize = ring_offsets.masked_tail(Self::capacity(tier));

        while i > gap_masked_idx {
            if let Some(elem) = cursor {
                cursor = Some(Self::replace(tier, i, elem));
            } else {
                cursor = Some(Self::take(tier, i));
            }

            i = Self::mask(tier, i.wrapping_sub(1));
        }

        if let Some(elem) = cursor {
            Self::set(tier, i, elem);
        }
    }

    pub fn remove_at_rank(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
    ) -> Result<T, TierError<T>> {
        let masked_rank = Self::mask(tier, ring_offsets.head().wrapping_add(rank));

        if Self::contains_masked_rank(tier, ring_offsets, masked_rank) {
            let elem = Self::take(tier, masked_rank);
            let cap = Self::capacity(tier);

            if masked_rank == ring_offsets.masked_head(cap) {
                ring_offsets.head_forward();
            } else if masked_rank == ring_offsets.masked_tail(cap) {
                ring_offsets.tail_backward();
            } else {
                Self::close_gap(tier, ring_offsets, masked_rank);
            }

            Ok(elem)
        } else {
            Err(TierError::TierRankOutOfBoundsError(rank))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::implicit::tier::*;

    fn prepare_slice(len: usize) -> Box<[MaybeUninit<usize>]> {
        let mut v = Vec::with_capacity(len);
        unsafe {
            v.set_len(len);
        }

        v.into_boxed_slice()
    }

    fn prepare_ring_offsets() -> ImplicitTierRingOffsets {
        ImplicitTierRingOffsets::default()
    }

    #[test]
    fn no_error_on_correct_tier_size() {
        let mut s = prepare_slice(4);
        assert_eq!(ImplicitTier::capacity(&s), 4);
    }

    #[test]
    fn insert_at_rank_shift_head() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());

        // [1, 3, 2, 0]
        assert!(ImplicitTier::insert_at_rank(&mut s, &mut ring_offsets, 1, 3).is_ok());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 0);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
    }

    #[test]
    fn insert_at_rank_shift_tail() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());

        // [0, 1, 3, 2]
        assert!(ImplicitTier::insert_at_rank(&mut s, &mut ring_offsets, 2, 3).is_ok());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 0);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn remove_at_rank_1() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s,)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s,)), 0);

        // [0, 2, 3, _]
        assert!(ImplicitTier::remove_at_rank(&mut s, &mut ring_offsets, 1).is_ok());
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 0);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 3);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_none());
    }

    #[test]
    fn remove_at_rank_2() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 0);

        // [_, 1, 2, 3]
        assert!(ImplicitTier::remove_at_rank(&mut s, &mut ring_offsets, 0).is_ok());
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 1);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 0);
        assert!(ImplicitTier::get(&s, &ring_offsets, 0).is_none());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 3);
    }
    #[test]
    fn shift_to_head_basic() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());

        // [1, 2, n, 0]
        ImplicitTier::shift_to_head(&mut s, &mut ring_offsets, 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 2);
        assert_ne!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 0);
    }

    #[test]
    fn shift_to_head_data_middle_1() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [n, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());
        assert!(ImplicitTier::pop_front(&mut s, &mut ring_offsets).is_ok());

        // [1, n, 2, n]
        ImplicitTier::shift_to_head(&mut s, &mut ring_offsets, 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_ne!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_none());
    }

    #[test]
    fn shift_to_head_data_middle_2() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [n, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());
        assert!(ImplicitTier::pop_front(&mut s, &mut ring_offsets).is_ok());

        // [1, 2, n, n]
        ImplicitTier::shift_to_head(&mut s, &mut ring_offsets, 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_ne!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_none());
    }

    #[test]
    fn shift_to_tail_nonwrapping() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());

        // [0, n, 1, 2]
        ImplicitTier::shift_to_tail(&mut s, &mut ring_offsets, 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 0);
        assert_ne!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn shift_to_tail_wrapping() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [3, n, 1, 2]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 0).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 2).is_ok());
        assert!(ImplicitTier::pop_front(&mut s, &mut ring_offsets).is_ok());
        assert!(ImplicitTier::pop_front(&mut s, &mut ring_offsets).is_ok());
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 3).is_ok());

        // [n, 3, 1, 2]
        ImplicitTier::shift_to_tail(&mut s, &mut ring_offsets, 0);
        assert_ne!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn push_and_pop() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        assert!(ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(ImplicitTier::capacity(&s)));
        assert_eq!(ring_offsets.len(), 0);
        assert_eq!(ImplicitTier::capacity(&s), 4);

        // [n, n, n, 0]
        assert!(ImplicitTier::push_front(&mut s, &mut ring_offsets, 0).is_ok());
        assert_eq!(ring_offsets.len(), 1);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 0);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 0);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 0);

        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        // [1, n, n, 0]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 1).is_ok());
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 0).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 1).unwrap(), 1);

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        // [1, n, 2, 0]
        assert!(ImplicitTier::push_front(&mut s, &mut ring_offsets, 2).is_ok());
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 2);

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        // [1, 3, 2, 0]
        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 3).is_ok());
        assert_eq!(ring_offsets.len(), 4);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 1).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 3);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 3).unwrap(), 3);

        assert!(!ring_offsets.is_empty());
        assert!(ring_offsets.is_full(ImplicitTier::capacity(&s)));

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        assert!(ImplicitTier::push_back(&mut s, &mut ring_offsets, 4).is_err());
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);

        // [1, 3, n, 0]
        let mut v = ImplicitTier::pop_front(&mut s, &mut ring_offsets);
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 2);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(ImplicitTier::capacity(&s)));
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_none());

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        // [1, n, n, 0]
        v = ImplicitTier::pop_back(&mut s, &mut ring_offsets);
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 3);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(ImplicitTier::capacity(&s)));
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 1).is_none());

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));

        // [1, n, 4, 0]
        assert!(ImplicitTier::push_front(&mut s, &mut ring_offsets, 4).is_ok());
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 4);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 4);

        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 1));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 2));
        assert!(ImplicitTier::is_valid_masked_index(&s, &ring_offsets, 3));
    }
}
