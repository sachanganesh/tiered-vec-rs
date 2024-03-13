use anyhow::Result;
use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

use super::tier::{Tier, TierError};

pub type TieredVecIndex = usize;

pub struct TieredVec<T> {
    pub(crate) max_tier_size: usize,
    pub(crate) tiers: Vec<Tier<T>>,
}

impl<T> TieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub(crate) fn new(initial_capacity: usize) -> Self {
        Self {
            max_tier_size: initial_capacity,
            tiers: Vec::with_capacity(initial_capacity),
        }
    }

    pub(crate) const fn tier_index(&self, idx: TieredVecIndex) -> usize {
        idx / self.max_tier_size
    }

    pub(crate) const fn tier_internal_index(&self, idx: TieredVecIndex) -> usize {
        idx % self.max_tier_size
    }

    pub fn get(&self, idx: TieredVecIndex) -> Option<&T> {
        if let Some(tier) = self.tiers.get(self.tier_index(idx)) {
            return tier.deref().get(self.tier_internal_index(idx));
        }

        None
    }

    pub fn get_mut(&mut self, idx: TieredVecIndex) -> Option<&mut T> {
        let t_idx = self.tier_index(idx);
        let i_idx = self.tier_internal_index(idx);

        if let Some(tier) = self.tiers.get_mut(t_idx) {
            return tier.deref_mut().get_mut(i_idx);
        }

        None
    }

    pub(crate) fn add_tier_and_insert(&mut self, data: T) {
        let mut tier = Tier::new(self.max_tier_size);
        tier.push_back(data)
            .expect("new tier did not accept elements");

        self.tiers.push(tier);
    }

    pub fn push(&mut self, data: T) -> TieredVecIndex {
        if let Some(tier) = self.tiers.last_mut() {
            match tier.push_back(data) {
                Ok(idx) => {
                    return (self.tiers.len() * self.max_tier_size) + idx;
                }

                Err(TierError::TierInsertionError(data)) => {
                    self.add_tier_and_insert(data);
                }

                _ => todo!(),
            }
        } else {
            self.add_tier_and_insert(data);
        }

        self.tiers.len() * self.max_tier_size
    }
}
