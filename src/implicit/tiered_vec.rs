use std::{fmt::Debug, mem::MaybeUninit};

use super::{tier::ImplicitTier, tier_ring_offsets::ImplicitTierRingOffsets};

pub struct ImplicitTieredVec<T> {
    offsets: Box<[ImplicitTierRingOffsets]>,
    buffer: Box<[MaybeUninit<T>]>,
}

impl<T> ImplicitTieredVec<T>
where
    T: Copy + Debug + Send + Sync + 'static,
{
    pub fn new(initial_capacity: usize) -> Self {
        assert!(initial_capacity.is_power_of_two());
        assert!(initial_capacity.ge(&4));

        let offsets = vec![ImplicitTierRingOffsets::default(); initial_capacity >> 1];

        let mut buffer = Vec::with_capacity(initial_capacity);
        unsafe {
            buffer.set_len(initial_capacity);
        }

        Self {
            offsets: offsets.into_boxed_slice(),
            buffer: buffer.into_boxed_slice(),
        }
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.buffer.len()
    }

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
    pub const fn is_empty(&self) -> bool {
        self.offsets[0].is_empty()
    }

    #[inline]
    pub const fn is_full(&self) -> bool {
        self.offsets[self.offsets.len()].is_full(self.offsets.len())
    }

    #[inline]
    const fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    const fn num_tiers(&self) -> usize {
        self.offsets.len()
    }

    #[inline]
    const fn tier_size(&self) -> usize {
        self.num_tiers()
    }

    #[inline]
    const fn tier_idx(&self, rank: usize) -> usize {
        rank / self.capacity()
    }

    fn get_tier_buffer(&self, idx: usize) -> &[MaybeUninit<T>] {
        &self.buffer[idx..idx + self.num_tiers()]
    }

    fn get_mut_tier_buffer(&mut self, idx: usize) -> &mut [MaybeUninit<T>] {
        let num_tiers = self.num_tiers();
        &mut self.buffer[idx..idx + num_tiers]
    }

    fn get_tier_offset(&self, rank: usize) -> &ImplicitTierRingOffsets {
        self.offsets
            .get(self.tier_idx(rank))
            .expect("tier offset does not exist at index")
    }

    fn get_mut_tier_offset(&mut self, rank: usize) -> &mut ImplicitTierRingOffsets {
        self.offsets
            .get_mut(self.tier_idx(rank))
            .expect("tier offset does not exist at index")
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        let tier_idx = self.tier_idx(rank);
        let tier = self.get_tier_buffer(tier_idx);
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
        let old_tier_size = self.tier_size();
        let new_tier_size = old_tier_size << 1;

        let mut new_buffer = Vec::with_capacity(new_tier_size).into_boxed_slice();

        for i in 0..new_tier_size {
            let old_offset_idx = i * 2;

            for j in old_offset_idx..old_offset_idx + 2 {
                let old_offset = &self.offsets[j];

                let start_idx = old_offset_idx * old_tier_size;
                let end_idx = start_idx + old_tier_size;
                let old_tier = &mut self.buffer[start_idx..end_idx];

                let new_tier = &mut new_buffer[i..i + new_tier_size];

                old_tier.rotate_left(old_offset.masked_head(old_tier_size));
                new_tier.clone_from_slice(&old_tier);
            }
        }

        self.offsets = vec![ImplicitTierRingOffsets::default(); new_tier_size].into_boxed_slice();
        self.buffer = new_buffer;
    }

    fn contract(&mut self) {
        todo!()
    }

    fn insert(&mut self, rank: usize, elem: T) {
        if self.is_full() {
            self.expand();
        }

        let offset_idx = self.tier_idx(rank);
        let last_offset_idx = self.tier_idx(self.len());
        let mut prev_popped = None;

        // pop-push phase
        for i in offset_idx..last_offset_idx + 1 {
            let start_idx = i * self.tier_size();
            let end_idx = start_idx + self.tier_size();

            let offsets = &mut self.offsets[i];
            let tier = &mut self.buffer[start_idx..end_idx];

            if let Ok(popped) = ImplicitTier::pop_back(tier, offsets) {
                if let Some(prev_elem) = prev_popped {
                    ImplicitTier::push_front(tier, offsets, prev_elem)
                        .expect("tier did not have space despite prior call to `pop_back`");
                }

                prev_popped = Some(popped);
            }
        }

        // shift phase
        let tier_idx = offset_idx * self.tier_size();
        let tier_idx_end = tier_idx + self.tier_size();

        let offset = &mut self.offsets[offset_idx];
        let tier = &mut self.buffer[tier_idx..tier_idx_end];
        ImplicitTier::insert(tier, offset, rank, elem)
            .expect("could not insert into tier at rank");
    }

    fn remove(&mut self, rank: usize) -> Option<T> {
        let mut prev_popped = None;

        // shift phase
        let offset_idx = self.tier_idx(rank);
        let last_offset_idx = self.tier_idx(self.len());
        let tier_idx = offset_idx * self.tier_size();
        let tier_idx_end = tier_idx + self.tier_size();

        let offsets = &mut self.offsets[offset_idx];
        let tier = &mut self.buffer[tier_idx..tier_idx_end];

        if let Ok(removed) = ImplicitTier::remove(tier, offsets, rank) {
            // pop-push phase
            for i in (tier_idx..last_offset_idx + 1).rev() {
                let start_idx = i * self.tier_size();
                let end_idx = start_idx + self.tier_size();

                let offsets = &mut self.offsets[i];
                let tier = &mut self.buffer[start_idx..end_idx];

                if let Ok(popped) = ImplicitTier::pop_back(tier, offsets) {
                    if let Some(prev_elem) = prev_popped {
                        ImplicitTier::push_front(tier, offsets, prev_elem)
                            .expect("tier did not have space despite prior call to `pop_back`");
                    }

                    prev_popped = Some(popped);
                }
            }

            // potentially contract

            return Some(removed);
        }

        return None;
    }
}
