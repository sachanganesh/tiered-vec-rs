use std::{fmt::Debug, marker::PhantomData, mem::MaybeUninit};

use super::tier_ring_offsets::ImplicitTierRingOffsets;

pub struct ImplicitTier<T>
where
    T: Debug,
{
    marker: PhantomData<T>,
}

impl<T> ImplicitTier<T>
where
    T: Debug,
{
    #[inline(always)]
    pub const fn capacity(tier: &[MaybeUninit<T>]) -> usize {
        tier.len()
    }

    #[inline]
    const fn mask(tier: &[MaybeUninit<T>], val: usize) -> usize {
        val & (Self::capacity(tier) - 1)
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
        if !Self::contains_masked_rank(tier, ring_offsets, idx) {
            return None;
        }

        let elem = &tier[idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub fn get_mut<'a>(
        tier: &'a mut [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        idx: usize,
    ) -> Option<&'a mut T> {
        if !Self::contains_masked_rank(tier, ring_offsets, idx) {
            return None;
        }

        let elem = &mut tier[idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank<'a>(
        tier: &'a [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> Option<&'a T> {
        let masked_rank = ring_offsets.masked_rank(rank, Self::capacity(tier));
        Self::get(tier, ring_offsets, masked_rank)
    }

    pub fn get_mut_by_rank<'a>(
        tier: &'a mut [MaybeUninit<T>],
        ring_offsets: &ImplicitTierRingOffsets,
        rank: usize,
    ) -> Option<&'a mut T> {
        Self::get_mut(
            tier,
            ring_offsets,
            ring_offsets.masked_rank(rank, Self::capacity(tier)),
        )
    }

    pub fn rotate_reset<'a>(
        tier: &'a mut [MaybeUninit<T>],
        ring_offsets: &'a mut ImplicitTierRingOffsets,
    ) {
        ring_offsets.set_tail(ring_offsets.len());

        let masked_head = ring_offsets.masked_head(Self::capacity(tier));
        tier.rotate_left(masked_head);

        ring_offsets.set_head(0);
    }

    #[inline]
    fn set(tier: &mut [MaybeUninit<T>], masked_idx: usize, elem: T) -> &mut T {
        tier[masked_idx].write(elem)
    }

    #[inline]
    fn take(tier: &mut [MaybeUninit<T>], masked_idx: usize) -> T {
        let elem = &mut tier[masked_idx];
        unsafe { elem.assume_init_read() }
    }

    #[inline]
    fn replace(tier: &mut [MaybeUninit<T>], masked_idx: usize, elem: T) -> T {
        let slot = &mut tier[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::new(elem)).assume_init() }
    }

    pub fn push_front(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) {
        let cap = Self::capacity(tier);
        assert!(!ring_offsets.is_full(cap));

        ring_offsets.head_backward();

        let idx = ring_offsets.masked_head(cap);
        Self::set(tier, idx, elem);
    }

    pub fn push_back(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) {
        let cap = Self::capacity(tier);
        assert!(!ring_offsets.is_full(cap));

        let idx = ring_offsets.masked_tail(cap);
        ring_offsets.tail_forward();

        Self::set(tier, idx, elem);
    }

    pub fn pop_front(tier: &mut [MaybeUninit<T>], ring_offsets: &mut ImplicitTierRingOffsets) -> T {
        assert!(!ring_offsets.is_empty());

        let idx = ring_offsets.masked_head(Self::capacity(tier));
        ring_offsets.head_forward();

        Self::take(tier, idx)
    }

    pub fn pop_back(tier: &mut [MaybeUninit<T>], ring_offsets: &mut ImplicitTierRingOffsets) -> T {
        assert!(!ring_offsets.is_empty());

        ring_offsets.tail_backward();
        let idx = ring_offsets.masked_tail(Self::capacity(tier));

        Self::take(tier, idx)
    }

    pub fn pop_push_front(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        elem: T,
    ) -> T {
        let cap = Self::capacity(tier);
        assert!(ring_offsets.is_full(cap));

        ring_offsets.head_backward();
        ring_offsets.tail_backward();
        let index = ring_offsets.masked_head(cap);

        Self::replace(tier, index, elem)
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

        while i != masked_tail {
            if let Some(curr_elem) = cursor {
                cursor = Some(Self::replace(tier, i, curr_elem));
            } else {
                cursor = Some(Self::take(tier, i));
            }

            i = Self::mask(tier, i.wrapping_add(1));
        }

        if let Some(curr_elem) = cursor {
            Self::set(tier, i, curr_elem);
        }

        ring_offsets.tail_forward();
    }

    pub fn insert(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
        elem: T,
    ) {
        let cap = Self::capacity(tier);
        assert!(!ring_offsets.is_full(cap));

        let masked_head = ring_offsets.masked_head(cap);
        let masked_tail = ring_offsets.masked_tail(cap);
        let masked_rank = ring_offsets.masked_rank(rank, cap);

        if masked_tail == masked_rank {
            Self::push_back(tier, ring_offsets, elem)
        } else if masked_head == masked_rank {
            Self::push_front(tier, ring_offsets, elem);
        } else {
            Self::shift_to_tail(tier, ring_offsets, masked_rank);

            Self::set(tier, masked_rank, elem);
        }
    }

    fn close_gap(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        gap_masked_idx: usize,
    ) {
        let mut cursor = None;

        ring_offsets.tail_backward();
        let mut i = ring_offsets.masked_tail(Self::capacity(tier));

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

    pub fn remove(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
        rank: usize,
    ) -> T {
        assert!(!ring_offsets.is_empty());

        let cap = Self::capacity(tier);
        let masked_rank = ring_offsets.masked_rank(rank, cap);

        let elem = Self::take(tier, masked_rank);

        if masked_rank == ring_offsets.masked_head(cap) {
            ring_offsets.head_forward();
        } else if masked_rank == ring_offsets.masked_tail(cap) {
            ring_offsets.tail_backward();
        } else {
            Self::close_gap(tier, ring_offsets, masked_rank);
        }

        elem
    }

    pub fn merge_neighbors(
        joint_tier: &mut [MaybeUninit<T>],
        first_ring_offsets: &mut ImplicitTierRingOffsets,
        second_ring_offsets: &mut ImplicitTierRingOffsets,
    ) {
        let tier_size = joint_tier.len() / 2;

        Self::rotate_reset(&mut joint_tier[..tier_size], first_ring_offsets);
        Self::rotate_reset(&mut joint_tier[tier_size..], second_ring_offsets);

        first_ring_offsets.set_tail(first_ring_offsets.len() + second_ring_offsets.len());
    }

    pub fn split_half(
        tier: &mut [MaybeUninit<T>],
        ring_offsets: &mut ImplicitTierRingOffsets,
    ) -> ImplicitTierRingOffsets {
        Self::rotate_reset(tier, ring_offsets);

        if ring_offsets.len() >= tier.len() / 2 {
            let count = ring_offsets.len();

            ring_offsets.set_tail(tier.len() / 2);

            ImplicitTierRingOffsets::new(0, count - tier.len() / 2)
        } else {
            ImplicitTierRingOffsets::new(0, 0)
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
        let s = prepare_slice(4);
        assert_eq!(ImplicitTier::capacity(&s), 4);
    }

    #[test]
    fn insert_shift_head() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);

        // [1, 3, 2, 0]
        ImplicitTier::insert(&mut s, &mut ring_offsets, 1, 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 0);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
    }

    #[test]
    fn insert_shift_tail() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, n]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);

        // [0, 1, 3, 2]
        ImplicitTier::insert(&mut s, &mut ring_offsets, 2, 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 0);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 1);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 2);
    }

    #[test]
    fn remove_1() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s,)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s,)), 0);

        // [0, 2, 3, _]
        ImplicitTier::remove(&mut s, &mut ring_offsets, 1);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 3);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 0);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 2);
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 3);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_none());
    }

    #[test]
    fn remove_2() {
        let mut s = prepare_slice(4);
        let mut ring_offsets = prepare_ring_offsets();

        // [0, 1, 2, 3]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 0);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 0);

        // [_, 1, 2, 3]
        ImplicitTier::remove(&mut s, &mut ring_offsets, 0);
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
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);

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
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);
        ImplicitTier::pop_front(&mut s, &mut ring_offsets);

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
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);
        ImplicitTier::pop_front(&mut s, &mut ring_offsets);

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
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);

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
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 0);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 2);
        ImplicitTier::pop_front(&mut s, &mut ring_offsets);
        ImplicitTier::pop_front(&mut s, &mut ring_offsets);
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 3);

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
        ImplicitTier::push_front(&mut s, &mut ring_offsets, 0);
        assert_eq!(ring_offsets.len(), 1);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 0);
        assert!(ImplicitTier::get(&s, &ring_offsets, 3).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 3).unwrap(), 0);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 0);

        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // [1, n, n, 0]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 1);
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 0).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 0).unwrap(), 1);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 1).unwrap(), 1);

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // [1, n, 2, 0]
        ImplicitTier::push_front(&mut s, &mut ring_offsets, 2);
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 2);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 2);

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // [1, 3, 2, 0]
        ImplicitTier::push_back(&mut s, &mut ring_offsets, 3);
        assert_eq!(ring_offsets.len(), 4);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 1).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 1).unwrap(), 3);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 3).unwrap(), 3);

        assert!(!ring_offsets.is_empty());
        assert!(ring_offsets.is_full(ImplicitTier::capacity(&s)));

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // ImplicitTier::push_back(&mut s, &mut ring_offsets, 4).is_err();
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);

        // [1, 3, n, 0]
        let mut v = ImplicitTier::pop_front(&mut s, &mut ring_offsets);
        assert_eq!(v, 2);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(ImplicitTier::capacity(&s)));
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 2);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_none());

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // [1, n, n, 0]
        v = ImplicitTier::pop_back(&mut s, &mut ring_offsets);
        assert_eq!(v, 3);
        assert!(!ring_offsets.is_empty());
        assert!(!ring_offsets.is_full(ImplicitTier::capacity(&s)));
        assert_eq!(ring_offsets.len(), 2);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 3);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 1).is_none());

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));

        // [1, n, 4, 0]
        ImplicitTier::push_front(&mut s, &mut ring_offsets, 4);
        assert_eq!(ring_offsets.len(), 3);
        assert_eq!(ring_offsets.masked_head(ImplicitTier::capacity(&s)), 2);
        assert_eq!(ring_offsets.masked_tail(ImplicitTier::capacity(&s)), 1);
        assert!(ImplicitTier::get(&s, &ring_offsets, 2).is_some());
        assert_eq!(*ImplicitTier::get(&s, &ring_offsets, 2).unwrap(), 4);
        assert_eq!(*ImplicitTier::get_by_rank(&s, &ring_offsets, 0).unwrap(), 4);

        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 0));
        assert!(!ImplicitTier::contains_masked_rank(&s, &ring_offsets, 1));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 2));
        assert!(ImplicitTier::contains_masked_rank(&s, &ring_offsets, 3));
    }
}
