use std::{fmt::Debug, mem::MaybeUninit};

use super::tier_ring_offsets::ImplicitTierRingOffsets;

pub struct ImplicitTieredVec<T> {
    offsets: Box<[ImplicitTierRingOffsets]>,
    buffer: Box<[MaybeUninit<T>]>,
}

impl<T> ImplicitTieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub fn new(initial_capacity: usize) -> Self {
        let offsets = vec![ImplicitTierRingOffsets::default(); initial_capacity];

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

    const fn tier_idx(&self, rank: usize) -> usize {
        rank / self.capacity()
    }

    fn get_mut_tier(&mut self, rank: usize) -> &mut [MaybeUninit<T>] {
        let tier_idx = self.tier_idx(rank);
        let cap = self.capacity();

        &mut self.buffer[tier_idx..tier_idx + cap]
    }

    fn get_mut_tier_offset(&mut self, rank: usize) -> &mut ImplicitTierRingOffsets {
        self.offsets
            .get_mut(self.tier_idx(rank))
            .expect("tier offset does not exist at index")
    }

    pub fn get_by_rank(&self) -> Option<&T> {
        todo!()
    }

    pub fn get_mut_by_rank(&mut self) -> Option<&mut T> {
        todo!()
    }

    fn expand(&mut self) {
        todo!()
    }

    fn contract(&mut self) {
        todo!()
    }

    fn insert(&mut self, rank: usize, elem: T) {
        todo!()
    }

    fn remove(&mut self, rank: usize, elem: T) {
        todo!()
    }
}
