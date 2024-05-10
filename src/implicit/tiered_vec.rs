use std::{
    fmt::Debug,
    mem::{self, MaybeUninit},
};

use crate::error::{TierError, TieredVectorError};

use super::{tier::ImplicitTier, tier_ring_offsets::ImplicitTierRingOffsets};

#[derive(Clone)]
pub struct ImplicitTieredVec<T>
where
    T: Copy,
{
    offsets: Vec<ImplicitTierRingOffsets>,
    buffer: Vec<MaybeUninit<T>>,
}

impl<T> ImplicitTieredVec<T>
where
    T: Copy + Debug + Send + Sync + 'static,
{
    pub fn new(initial_tier_size: usize) -> Self {
        assert!(initial_tier_size.is_power_of_two());
        assert!(initial_tier_size.ge(&2));

        let offsets = vec![ImplicitTierRingOffsets::default(); initial_tier_size];

        let capacity = initial_tier_size.pow(2);
        let mut buffer = Vec::with_capacity(capacity);
        unsafe {
            buffer.set_len(capacity);
        }

        Self { offsets, buffer }
    }

    pub fn with_minimum_capacity(min_capacity: usize) -> Self {
        assert!(min_capacity.ge(&4));

        let mut capacity = min_capacity;
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

        let tier_size = capacity >> shift_count;

        let offsets = vec![ImplicitTierRingOffsets::default(); tier_size];

        let mut buffer = Vec::with_capacity(capacity);
        unsafe {
            buffer.set_len(capacity);
        }

        Self { offsets, buffer }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        let mut l = 0;

        for offset in self.offsets.iter() {
            let offset_len = offset.len();

            if offset_len == 0 {
                break;
            }

            l += offset_len;
        }

        return l;
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
    pub fn tier_size(&self) -> usize {
        self.num_tiers()
    }

    #[inline]
    fn tier_idx(&self, rank: usize) -> usize {
        rank / self.tier_size()
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        let tier_idx = self.tier_idx(rank);

        let num_tiers = self.num_tiers();
        let start_idx = tier_idx * num_tiers;
        let end_idx = start_idx + num_tiers;

        let tier = &self.buffer[start_idx..end_idx];
        let ring_offsets = &self.offsets[tier_idx];
        ImplicitTier::get_by_rank(tier, ring_offsets, rank)
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        let tier_size = self.tier_size();
        let tier_idx = self.tier_idx(rank);

        let ring_offsets = &self.offsets[tier_idx];
        let tier = &mut self.buffer[tier_idx..tier_idx + tier_size];

        ImplicitTier::get_mut_by_rank(tier, ring_offsets, rank)
    }

    fn expand(&mut self) {
        let curr_tier_size = self.tier_size();
        let new_tier_size = curr_tier_size << 1;

        for i in 0..(curr_tier_size / 2) {
            let mut second_ring_offsets = self.offsets.remove(i + 1);

            let start_idx = i * curr_tier_size;
            let end_idx = start_idx + (curr_tier_size * 2);
            ImplicitTier::merge_neighbors(
                &mut self.buffer[start_idx..end_idx],
                &mut self.offsets[i],
                &mut second_ring_offsets,
            );
        }

        for _ in 0..(new_tier_size - (curr_tier_size / 2)) {
            self.offsets.push(Default::default());
        }
        self.buffer
            .resize(new_tier_size.pow(2), MaybeUninit::uninit());
    }

    fn try_contract(&mut self, num_entries: usize) {
        // only contract well below capacity to cull repeated alloc/free of memory upon reinsertion/redeletion
        if num_entries < self.capacity() / 8 {
            let curr_tier_size = self.tier_size();
            let new_tier_size = curr_tier_size >> 1;

            let split_idx = new_tier_size >> 1;
            let _ = self.offsets.split_off(split_idx);

            let end_idx = new_tier_size;
            for i in (0..end_idx).step_by(2) {
                let old_start_idx = i * curr_tier_size;
                let old_end_idx = old_start_idx + curr_tier_size;
                let old_tier = &mut self.buffer[old_start_idx..old_end_idx];
                let old_ring_offsets = &mut self.offsets[i];

                let new_ring_offsets = ImplicitTier::split_half(old_tier, old_ring_offsets);
                self.offsets.insert(i + 1, new_ring_offsets);
            }

            let _ = self.buffer.split_off(split_idx * curr_tier_size);

            assert_eq!(self.offsets.len(), new_tier_size);
        }
    }

    pub fn insert(&mut self, rank: usize, elem: T) -> Result<usize, TieredVectorError<T>> {
        let num_entries = self.len();
        if rank > num_entries {
            return Err(TieredVectorError::TieredVectorOutofBoundsInsertionError(
                rank, elem,
            ));
        }

        if num_entries == self.capacity() {
            self.expand();
        }

        let offset_idx = self.tier_idx(rank);
        let tier_size = self.tier_size();
        let mut prev_popped = None;

        // pop-push phase
        if self.offsets[offset_idx].is_full(tier_size) {
            for i in offset_idx..tier_size {
                let start_idx = i * tier_size;
                let end_idx = start_idx + tier_size;
                let tier = &mut self.buffer[start_idx..end_idx];
                let ring_offsets = &mut self.offsets[i];

                if ring_offsets.is_full(tier.len()) {
                    if let Ok(popped) = ImplicitTier::pop_front(tier, ring_offsets) {
                        if let Some(prev_elem) = prev_popped {
                            ImplicitTier::push_back(tier, ring_offsets, prev_elem).expect(
                                "tier did not have space despite prior call to `pop_front`",
                            );
                        }

                        prev_popped = Some(popped);
                    }
                } else {
                    if let Some(prev_elem) = prev_popped.take() {
                        ImplicitTier::push_back(tier, ring_offsets, prev_elem)
                            .expect("tier did not have space despite prior call to `pop_front`");
                    }
                }
            }
        }

        // shift phase
        let tier_idx = offset_idx * tier_size;
        let tier_idx_end = tier_idx + tier_size;

        let tier = &mut self.buffer[tier_idx..tier_idx_end];
        let ring_offsets = &mut self.offsets[offset_idx];
        ImplicitTier::insert(tier, ring_offsets, rank, elem)
            .expect("could not insert into tier at rank");

        Ok(rank)
    }

    pub fn remove(&mut self, rank: usize) -> Result<T, TieredVectorError<T>> {
        let num_entries = self.len();
        if rank > num_entries {
            return Err(TieredVectorError::TieredVectorRankOutOfBoundsError(rank));
        }

        self.try_contract(num_entries);

        // get last valid tier
        // if target_tier is same as last_tier
        //      remove element and shift remaining over
        // else
        //      let t = target_tier
        //      let mut u = t.next_tier()
        //      let removed = t.remove(masked_rank)
        //
        //      while u != last_tier:
        //          mem::swap(t.tail(), u.head())
        //
        //          t.tail_forward()
        //          t = u
        //          u = t.next_tier()
        //
        //      mem::swap(t.tail(), u.head())
        //      t.tail_forward()
        //      u.head_forward()
        //
        //      return removed

        // println!("BEFORE");
        // for (i, o) in self.offsets.iter().enumerate() {
        //     println!("offset {}: {:?}", i, o);
        // }

        let mut offset_idx = self.tier_idx(rank);

        let tier_size = self.tier_size();
        let tier_idx = offset_idx * tier_size;
        let tier_idx_end = tier_idx + tier_size;

        let target_tier = &mut self.buffer[tier_idx..tier_idx_end];
        let target_ring_offsets = &mut self.offsets[offset_idx];

        match ImplicitTier::remove(target_tier, target_ring_offsets, rank) {
            Err(TierError::TierEmptyError) => Err(TieredVectorError::TieredVectorEmptyError),
            Err(TierError::TierRankOutOfBoundsError(r)) => {
                Err(TieredVectorError::TieredVectorRankOutOfBoundsError(r))
            }
            Err(_) => unreachable!(),

            Ok(removed) => {
                // println!("DURING");
                // for (i, o) in self.offsets.iter().enumerate() {
                //     println!("offset {}: {:?}", i, o);
                // }

                let last_offset_idx = self.tier_idx(num_entries - 1);

                if offset_idx != last_offset_idx {
                    //      let t = target_tier
                    //      let mut u = t.next_tier()
                    //      let removed = t.remove(masked_rank)
                    //
                    //      while u != last_tier:
                    //          mem::swap(t.tail(), u.head())
                    //
                    //          t.tail_forward()
                    //          t = u
                    //          u = t.next_tier()
                    //
                    //      mem::swap(t.tail(), u.head())
                    //      t.tail_forward()
                    //      u.head_forward()
                    //
                    //      return removed

                    for cursor_offset_idx in offset_idx..last_offset_idx {
                        let next_offset_idx = cursor_offset_idx + 1;

                        let cursor_offsets = &self.offsets[cursor_offset_idx];
                        let next_offsets = &self.offsets[next_offset_idx];

                        let cursor_idx =
                            (cursor_offset_idx * tier_size) + cursor_offsets.masked_tail(tier_size);
                        let next_idx =
                            (next_offset_idx * tier_size) + next_offsets.masked_head(tier_size);

                        // println!(
                        //     "swapping {} with {} / {} / {}",
                        //     cursor_idx,
                        //     next_idx,
                        //     num_entries - 1,
                        //     self.buffer.len()
                        // );
                        self.buffer.swap(cursor_idx, next_idx);
                    }

                    self.offsets[offset_idx].tail_forward();
                    self.offsets[last_offset_idx].head_forward();

                    // println!("AFTER");
                    // for (i, o) in self.offsets.iter().enumerate() {
                    //     println!("offset {}: {:?}", i, o);
                    // }
                }

                Ok(removed)
            }
        }

        //============================================

        // let mut prev_popped = None;

        // // shift phase
        // let offset_idx = self.tier_idx(rank);

        // let tier_size = self.tier_size();
        // let tier_idx = offset_idx * tier_size;
        // let tier_idx_end = tier_idx + tier_size;

        // let target_tier = &mut self.buffer[tier_idx..tier_idx_end];
        // let target_ring_offsets = &mut self.offsets[offset_idx];

        // match ImplicitTier::remove(target_tier, target_ring_offsets, rank) {
        //     Err(TierError::TierEmptyError) => Err(TieredVectorError::TieredVectorEmptyError),
        //     Err(TierError::TierRankOutOfBoundsError(r)) => {
        //         Err(TieredVectorError::TieredVectorRankOutOfBoundsError(r))
        //     }
        //     Err(_) => unreachable!(),

        //     Ok(removed) => {
        //         let last_tier_idx = self.tier_idx(num_entries);

        //         // pop-push phase
        //         for i in (offset_idx + 1..last_tier_idx + 1).rev() {
        //             let start_idx = i * tier_size;
        //             let end_idx = start_idx + tier_size;

        //             let tier = &mut self.buffer[start_idx..end_idx];
        //             let ring_offsets = &mut self.offsets[i];

        //             if let Ok(popped) = ImplicitTier::pop_front(tier, ring_offsets) {
        //                 if let Some(prev_elem) = prev_popped {
        //                     ImplicitTier::push_back(tier, ring_offsets, prev_elem)
        //                         .expect("tier did not have space despite prior call to `pop_back`");
        //                 }

        //                 prev_popped = Some(popped);
        //             }
        //         }

        //         if let Some(popped) = prev_popped {
        //             let target_tier = &mut self.buffer[tier_idx..tier_idx_end];
        //             let target_ring_offsets = &mut self.offsets[offset_idx];

        //             ImplicitTier::push_back(target_tier, target_ring_offsets, popped)
        //                 .expect("tier did not have space despite prior removal");
        //         }

        //         Ok(removed)
        //     }
        // }
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
        assert_eq!(t.tier_size(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn with_minimum_capacity() {
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::with_minimum_capacity(4);
        assert_eq!(4, t.capacity());
        assert_eq!(2, t.tier_size());

        t = ImplicitTieredVec::with_minimum_capacity(8);
        assert_eq!(16, t.capacity());
        assert_eq!(4, t.tier_size());

        t = ImplicitTieredVec::with_minimum_capacity(128);
        assert_eq!(256, t.capacity());
        assert_eq!(16, t.tier_size());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);
        assert_eq!(t.tier_size(), size);

        for i in 0..size {
            assert!(t.insert(i, i * 2).is_ok());
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
            assert!(t.insert(i, i).is_ok());
            assert_eq!(*t.get_by_rank(i).unwrap(), i);
        }
        assert_eq!(t.tier_size(), size);
        assert_eq!(t.len(), size * size);
        assert!(t.is_full());

        assert!(t.insert(size * size, size * size).is_ok());
        assert_eq!(t.tier_size(), size * 2);
        assert_eq!(t.len(), (size * size) + 1);
        assert!(!t.is_full());

        for i in 0..((size * size) + 1) {
            let result = t.get_by_rank(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i);
        }
    }

    #[test]
    fn remove_and_contract() {
        let size = 16;
        let mut t: ImplicitTieredVec<usize> = ImplicitTieredVec::new(size);
        assert_eq!(t.capacity(), size * size);

        for i in 0..size * size / 8 {
            // size / 8 {
            assert!(t.insert(i, i).is_ok());
            assert_eq!(*t.get_by_rank(i).unwrap(), i);
        }
        assert_eq!(t.tier_size(), size);
        assert_eq!(t.len(), size * size / 8);
        assert_eq!(t.capacity(), size * size);

        assert!(t.remove(0).is_ok());

        assert_eq!(*t.get_by_rank(0).unwrap(), 1);
        assert_eq!(t.len(), (size * size / 8) - 1);
        assert_eq!(t.capacity(), size * size);

        // contract
        assert!(t.remove(0).is_ok());

        assert_eq!(*t.get_by_rank(0).unwrap(), 2);
        assert_eq!(t.len(), (size * size / 8) - 2);
        assert_eq!(t.capacity(), size * size / 4);
    }
}
