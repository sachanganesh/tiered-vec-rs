use std::{fmt::Debug, mem::MaybeUninit};

use super::{tier::ImplicitTier, tier_ring_offsets::ImplicitTierRingOffsets};

pub struct ImplicitTieredVec<T> {
    offsets: Vec<ImplicitTierRingOffsets>,
    buffer: Vec<MaybeUninit<T>>,
    tier_log: usize,
    len: usize,
}

impl<T> ImplicitTieredVec<T>
where
    T: Clone + Debug,
{
    pub fn new(tier_capacity: usize) -> Self {
        assert!(tier_capacity.is_power_of_two());
        assert!(tier_capacity.ge(&2));

        let offsets = vec![ImplicitTierRingOffsets::default(); tier_capacity];

        let capacity = tier_capacity.pow(2);
        let mut buffer = Vec::with_capacity(capacity);
        unsafe {
            buffer.set_len(capacity);
        }

        Self {
            offsets,
            buffer,
            tier_log: tier_capacity.ilog2() as usize,
            len: 0,
        }
    }

    pub fn with_minimum_capacity(mut capacity: usize) -> Self {
        assert!(capacity.ge(&4));

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

        let offsets = vec![ImplicitTierRingOffsets::default(); tier_capacity];

        let mut buffer = Vec::with_capacity(capacity);
        unsafe {
            buffer.set_len(capacity);
        }

        Self {
            offsets,
            buffer,
            tier_log: tier_capacity.ilog2() as usize,
            len: 0,
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offsets[0].is_empty()
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.offsets[self.offsets.len() - 1].is_full(self.offsets.len())
    }

    #[inline]
    fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    fn num_tiers(&self) -> usize {
        self.offsets.len()
    }

    #[inline]
    pub fn tier_capacity(&self) -> usize {
        self.num_tiers()
    }

    #[inline]
    fn tier_index(&self, rank: usize) -> usize {
        rank >> self.tier_log
    }

    #[inline]
    fn tier_buffer_index(&self, tier_index: usize) -> usize {
        tier_index << self.tier_log
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        let tier_index = self.tier_index(rank);

        let start_index = self.tier_buffer_index(tier_index);
        let end_index = start_index + self.tier_capacity();

        let tier = &self.buffer[start_index..end_index];
        let ring_offsets = &self.offsets[tier_index];
        ImplicitTier::get_by_rank(tier, ring_offsets, rank)
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        let tier_index = self.tier_index(rank);

        let start_index = self.tier_buffer_index(tier_index);
        let end_index = start_index + self.tier_capacity();

        let ring_offsets = &self.offsets[tier_index];
        let tier = &mut self.buffer[start_index..end_index];

        ImplicitTier::get_mut_by_rank(tier, ring_offsets, rank)
    }

    fn expand(&mut self) {
        let curr_tier_capacity = self.tier_capacity();
        let new_tier_capacity = curr_tier_capacity << 1;

        for i in 0..(curr_tier_capacity / 2) {
            let mut second_ring_offsets = self.offsets.remove(i + 1);

            let start_index = i * curr_tier_capacity;
            let end_index = start_index + (curr_tier_capacity * 2);
            ImplicitTier::merge_neighbors(
                &mut self.buffer[start_index..end_index],
                &mut self.offsets[i],
                &mut second_ring_offsets,
            );
        }

        for _ in 0..(new_tier_capacity - (curr_tier_capacity / 2)) {
            self.offsets.push(Default::default());
        }
        self.buffer
            .resize_with(new_tier_capacity.pow(2), MaybeUninit::uninit);
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        assert!(index <= self.len());

        if self.is_full() {
            self.expand();
        }

        let tier_capacity = self.tier_capacity();
        let offset_index = self.tier_index(index);
        let start_index = self.tier_buffer_index(offset_index);
        let end_index = start_index + self.tier_capacity();

        if !self.offsets[offset_index].is_full(self.tier_capacity()) {
            ImplicitTier::insert(
                &mut self.buffer[start_index..end_index],
                &mut self.offsets[offset_index],
                index,
                elem,
            );
            self.len += 1;

            return;
        }

        let last_tier_index = self.tier_index(self.len() - 1);

        let mut start_index = self.tier_buffer_index(offset_index);
        let mut tier = &mut self.buffer[start_index..start_index + tier_capacity];
        let mut ring_offsets = &mut self.offsets[offset_index];

        let mut prev_popped = Some(ImplicitTier::pop_back(tier, ring_offsets));
        ImplicitTier::insert(tier, ring_offsets, index, elem);

        for i in offset_index + 1..last_tier_index {
            start_index = self.tier_buffer_index(i);
            tier = &mut self.buffer[start_index..start_index + tier_capacity];
            ring_offsets = &mut self.offsets[i];

            let prev_elem = prev_popped.take().expect("loop should always pop a value");
            prev_popped = Some(ImplicitTier::pop_push_front(tier, ring_offsets, prev_elem));
        }

        start_index = self.tier_buffer_index(last_tier_index);
        tier = &mut self.buffer[start_index..start_index + tier_capacity];
        ring_offsets = &mut self.offsets[last_tier_index];

        if ring_offsets.is_full(tier_capacity) {
            let prev_elem = prev_popped.take().expect("loop should always pop a value");
            prev_popped = Some(ImplicitTier::pop_push_front(tier, ring_offsets, prev_elem));

            start_index = self.tier_buffer_index(last_tier_index + 1);
            tier = &mut self.buffer[start_index..start_index + tier_capacity];
            ring_offsets = &mut self.offsets[last_tier_index + 1];
        }

        ImplicitTier::push_front(
            tier,
            ring_offsets,
            prev_popped.take().expect("loop should always pop a value"),
        );
        self.len += 1;
    }
}

impl<T> Clone for ImplicitTieredVec<T>
where
    T: Copy,
{
    fn clone(&self) -> Self {
        Self {
            offsets: self.offsets.clone(),
            buffer: self.buffer.clone(),
            tier_log: self.tier_log,
            len: self.len,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn error_on_non_power_of_two_size() {
        let _t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(5);
    }

    #[test]
    #[should_panic]
    fn error_on_small_size() {
        let _t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(1);
    }

    #[test]
    fn no_error_on_correct_size() {
        let size = 4;
        let t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), size * size);
        assert_eq!(t.tier_capacity(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn with_minimum_capacity() {
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::with_minimum_capacity(4);
        assert_eq!(4, t.capacity());
        assert_eq!(2, t.tier_capacity());

        t = ImplicitTieredVec::with_minimum_capacity(8);
        assert_eq!(16, t.capacity());
        assert_eq!(4, t.tier_capacity());

        t = ImplicitTieredVec::with_minimum_capacity(128);
        assert_eq!(256, t.capacity());
        assert_eq!(16, t.tier_capacity());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);
        assert_eq!(t.tier_capacity(), size);

        for i in 0..size {
            t.insert(i, i * 2);
            assert_eq!(t.len(), i + 1);
        }

        for i in 0..size {
            let result = t.get_by_rank(i);
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
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);

        for i in 0..size * size {
            t.insert(i, i);
            assert_eq!(*t.get_by_rank(i).unwrap(), i);
        }
        assert_eq!(t.tier_capacity(), size);
        assert_eq!(t.len(), size * size);
        assert!(t.is_full());

        t.insert(size * size, size * size);
        assert_eq!(t.tier_capacity(), size * 2);
        assert_eq!(t.len(), (size * size) + 1);
        assert!(!t.is_full());

        for i in 0..((size * size) + 1) {
            let result = t.get_by_rank(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i);
        }
    }
}
