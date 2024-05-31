use std::ops::{Index, IndexMut};

use super::tier::Tier;

#[derive(Clone)]
pub struct LinkedTieredVec<T> {
    tiers: Vec<Tier<T>>,
    tier_log: usize,
    len: usize,
}

impl<T> LinkedTieredVec<T> {
    pub fn new(tier_capacity: usize) -> Self {
        assert!(tier_capacity.is_power_of_two());
        assert!(tier_capacity.ge(&2));

        let mut tiers = Vec::with_capacity(tier_capacity);
        for _ in 0..tier_capacity {
            tiers.push(Tier::new(tier_capacity));
        }

        Self {
            tiers,
            tier_log: tier_capacity.ilog2() as usize,
            len: 0,
        }
    }

    pub fn with_capacity(mut capacity: usize) -> Self {
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
        Self::new(tier_capacity)
    }

    #[inline]
    pub fn tier_capacity(&self) -> usize {
        self.tiers.len()
    }

    #[inline]
    fn num_tiers(&self) -> usize {
        self.tier_capacity()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.tier_capacity().pow(2)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn mask(&self, val: usize) -> usize {
        val & (self.tier_capacity() - 1)
    }

    #[inline]
    fn tier_index(&self, rank: usize) -> usize {
        rank >> self.tier_log
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    pub fn get(&self, rank: usize) -> Option<&T> {
        self.tiers.get(self.tier_index(rank))?.get_by_rank(rank)
    }

    pub fn get_mut(&mut self, rank: usize) -> Option<&mut T> {
        let tier_idx = self.tier_index(rank);

        self.tiers.get_mut(tier_idx)?.get_by_rank_mut(rank)
    }

    fn expand(&mut self) {
        let curr_tier_size = self.tier_capacity();
        let new_tier_size = self.tier_capacity() << 1;

        for i in 0..(curr_tier_size / 2) {
            let second_tier = self.tiers.remove(i + 1);
            let first_tier = &mut self.tiers[i];

            first_tier.merge(second_tier);
        }

        for _ in 0..(new_tier_size - (curr_tier_size / 2)) {
            self.tiers.push(Tier::new(new_tier_size));
        }

        self.tier_log = new_tier_size.ilog2() as usize;
    }

    fn try_contract(&mut self, num_entries: usize) {
        // only contract well below capacity to cull repeated alloc/free of memory upon reinsertion/redeletion
        if num_entries < self.capacity() / 8 {
            let new_tier_size = self.tier_capacity() >> 1;
            let _ = self.tiers.split_off(new_tier_size >> 1);

            let end_idx = new_tier_size;
            for i in (0..end_idx).step_by(2) {
                let old_tier = self.tiers.get_mut(i).expect("tier at index does not exist");
                let half_tier = old_tier.split_half();

                assert_eq!(half_tier.capacity(), new_tier_size);
                self.tiers.insert(i + 1, half_tier);
            }

            self.tier_log = new_tier_size.ilog2() as usize;
            assert_eq!(self.tiers.len(), new_tier_size);
        }
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        assert!(index <= self.len());

        if self.is_full() {
            self.expand();
        }

        let tier_index = self.tier_index(index);

        if !self.tiers[tier_index].is_full() {
            self.tiers[tier_index].insert(index, elem);
            self.len += 1;

            return;
        }

        let last_tier_index = self.tier_index(self.len() - 1);

        let mut tier = &mut self.tiers[tier_index];
        let mut prev_popped = Some(tier.pop_back());
        tier.insert(index, elem);

        for i in tier_index + 1..last_tier_index {
            tier = &mut self.tiers[i];

            let prev_elem = prev_popped.take().expect("loop should always pop a value");
            prev_popped = Some(tier.pop_push_front(prev_elem));
        }

        if tier_index == last_tier_index {
            tier = &mut self.tiers[last_tier_index + 1];
        } else {
            tier = &mut self.tiers[last_tier_index];

            if tier.is_full() {
                let prev_elem = prev_popped.take().expect("loop should always pop a value");
                prev_popped = Some(tier.pop_push_front(prev_elem));

                tier = &mut self.tiers[last_tier_index + 1];
            }
        }

        tier.push_front(prev_popped.take().expect("loop should always pop a value"));
        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> T {
        assert!(index < self.len());

        let tier_index = self.tier_index(index);

        if !self.tiers[tier_index].is_full() {
            self.len -= 1;
            return self.tiers[tier_index].remove(index);
        }

        let last_tier_index = self.tier_index(self.len() - 1);
        let mut prev_popped = Some(self.tiers[last_tier_index].pop_front());

        for i in (tier_index + 1..last_tier_index).rev() {
            let tier = &mut self.tiers[i];

            let prev_elem = prev_popped.take().expect("loop should always pop a value");
            prev_popped = Some(tier.pop_push_back(prev_elem));
        }

        let tier = &mut self.tiers[tier_index];
        let elem = tier.remove(index);
        tier.push_back(prev_popped.take().expect("loop should always pop a value"));

        self.len -= 1;
        return elem;
    }

    pub fn push(&mut self, elem: T) {
        if self.is_full() {
            self.expand();
        }

        let index = self.tier_index(self.len());
        let tier = &mut self.tiers[index];
        assert!(!tier.is_full());

        tier.push_back(elem);
        self.len += 1;
    }

    pub fn pop(&mut self) -> T {
        assert!(!self.is_empty());

        let index = self.tier_index(self.len() - 1);
        let tier = &mut self.tiers[index];
        assert!(!tier.is_empty());

        let elem = tier.pop_back();

        self.len -= 1;
        return elem;
    }
}

impl<T> Index<usize> for LinkedTieredVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len());
        &self.tiers[self.tier_index(index)][index]
    }
}

impl<T> IndexMut<usize> for LinkedTieredVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len());

        let tier_index = self.tier_index(index);
        &mut self.tiers[tier_index][index]
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    #[should_panic]
    fn error_on_non_power_of_two_size() {
        let _t: LinkedTieredVec<usize> = LinkedTieredVec::new(5);
    }

    #[test]
    #[should_panic]
    fn error_on_small_size() {
        let _t: LinkedTieredVec<usize> = LinkedTieredVec::new(1);
    }

    #[test]
    fn no_error_on_correct_size() {
        let size = 4;
        let t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), size * size);
        assert_eq!(t.tier_capacity(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn with_minimum_capacity() {
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::with_capacity(4);
        assert_eq!(4, t.capacity());
        assert_eq!(2, t.tier_capacity());

        t = LinkedTieredVec::with_capacity(8);
        assert_eq!(16, t.capacity());
        assert_eq!(4, t.tier_capacity());

        t = LinkedTieredVec::with_capacity(128);
        assert_eq!(256, t.capacity());
        assert_eq!(16, t.tier_capacity());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);
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
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);

        for i in 0..size * size {
            t.insert(i, i);
            assert_eq!(t[i], i);
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
            assert_eq!(t[i], i);
        }
    }

    #[test]
    fn expand_2() {
        let size = 4;
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);

        for i in 0..1_000 {
            t.insert(0, i);
            assert_eq!(t[0], i);

            for j in 1..t.len() {
                assert_eq!(t[j], i - j);
            }
        }
    }

    #[test]
    fn remove() {
        let size = 16;
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);
        assert_eq!(t.capacity(), size * size);

        for i in 0..size * size / 8 {
            t.insert(i, i);
            assert_eq!(t[i], i);
        }
        assert_eq!(t.tier_capacity(), size);
        assert_eq!(t.len(), size * size / 8);
        assert_eq!(t.capacity(), size * size);

        t.remove(0);

        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(t.len(), (size * size / 8) - 1);

        t.remove(0);
        assert_eq!(*t.get(0).unwrap(), 2);
        assert_eq!(t.len(), (size * size / 8) - 2);
    }

    #[test]
    fn contract() {}
}
