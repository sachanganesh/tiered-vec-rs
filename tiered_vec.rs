use anyhow::Result;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

use super::tier::{Tier, TierError};

pub type TieredVecIndex = usize;

pub struct TieredVec<T> {
    pub(crate) tier_size: usize,
    pub(crate) tiers: Vec<Tier<T>>,
}

impl<T> TieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub(crate) fn new(initial_capacity: usize) -> Self {
        Self {
            tier_size: initial_capacity,
            tiers: Vec::with_capacity(initial_capacity),
        }
    }

    pub(crate) const fn tier_index(&self, idx: TieredVecIndex) -> usize {
        idx / self.tier_size
    }

    // pub(crate) const fn tier_internal_index(&self, idx: TieredVecIndex) -> usize {
    //     idx % self.tier_size
    // }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        self.tiers.get(self.tier_index(rank))?.get_by_rank(rank)
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        let tier_idx = self.tier_index(rank);

        self.tiers.get_mut(tier_idx)?.get_mut_by_rank(rank)
    }

    fn pop_push(&mut self, tier: &mut Tier<T>, elem: T) {}

    fn insert(&mut self, rank: usize, elem: T) {
        let tier_idx = self.tier_index(rank);
        let mut prev_popped = None;

        // pop-push phase
        for i in tier_idx..self.tier_size {
            let tier = self
                .tiers
                .get_mut(tier_idx)
                .expect("tier at index does not exist");

            if let Ok(popped) = tier.pop_back() {
                if let Some(prev_elem) = prev_popped {
                    tier.push_front(prev_elem)
                        .expect("tier did not have space despite prior call to `pop_back`");
                }

                prev_popped = Some(popped);
            } else {
                break;
            }
        }

        // shift phase
        
    }

    // pub fn get(&self, idx: TieredVecIndex) -> Option<&T> {
    //     if let Some(tier) = self.tiers.get(self.tier_index(idx)) {
    //         return tier.deref().get(self.tier_internal_index(idx));
    //     }

    //     None
    // }

    // pub fn get_mut(&mut self, idx: TieredVecIndex) -> Option<&mut T> {
    //     let t_idx = self.tier_index(idx);
    //     let i_idx = self.tier_internal_index(idx);

    //     if let Some(tier) = self.tiers.get_mut(t_idx) {
    //         return tier.deref_mut().get_mut(i_idx);
    //     }

    //     None
    // }

    // pub(crate) fn add_tier_and_insert(&mut self, data: T) {
    //     let mut tier = Tier::new(self.tier_size);
    //     tier.push_back(data)
    //         .expect("new tier did not accept elements");

    //     self.tiers.push(tier);
    // }

    // pub fn push(&mut self, data: T) -> TieredVecIndex {
    //     if let Some(tier) = self.tiers.last_mut() {
    //         match tier.push_back(data) {
    //             Ok(idx) => {
    //                 return (self.tiers.len() * self.tier_size) + idx;
    //             }

    //             Err(TierError::TierInsertionError(data)) => {
    //                 self.add_tier_and_insert(data);
    //             }

    //             _ => todo!(),
    //         }
    //     } else {
    //         self.add_tier_and_insert(data);
    //     }

    //     self.tiers.len() * self.tier_size
    // }
}
