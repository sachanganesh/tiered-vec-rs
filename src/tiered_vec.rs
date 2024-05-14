use std::fmt::{Debug, Write};

use super::error::{TierError, TieredVectorError};
use super::tier::Tier;

#[derive(Clone)]
pub struct LinkedTieredVec<T>
where
    T: Clone + Debug,
{
    pub(crate) tiers: Vec<Tier<T>>,
}

impl<T> LinkedTieredVec<T>
where
    T: Clone + Debug,
{
    pub fn new(initial_tier_size: usize) -> Self {
        assert!(initial_tier_size.is_power_of_two());
        assert!(initial_tier_size.ge(&2));

        let mut tiers = Vec::with_capacity(initial_tier_size);
        for _ in 0..initial_tier_size {
            tiers.push(Tier::new(initial_tier_size));
        }

        Self { tiers }
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
        let mut tiers = Vec::with_capacity(tier_size);
        for _ in 0..tier_size {
            tiers.push(Tier::new(tier_size));
        }

        Self { tiers }
    }

    #[inline(always)]
    pub fn tier_size(&self) -> usize {
        self.tiers.len()
    }

    pub(crate) fn tier_idx(&self, idx: usize) -> usize {
        idx / self.tier_size()
    }

    pub fn capacity(&self) -> usize {
        self.tier_size() * self.tiers.len()
    }

    pub fn len(&self) -> usize {
        let mut l = 0;
        for t in &self.tiers {
            let curr_len = t.len();

            if curr_len == 0 {
                return l;
            }

            l += curr_len;
        }

        return l;
    }

    pub fn is_empty(&self) -> bool {
        self.tiers
            .get(0)
            .expect("first tier is not initialized")
            .is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.tiers[self.tiers.len() - 1].is_full()
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        self.tiers.get(self.tier_idx(rank))?.get_by_rank(rank)
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        let tier_idx = self.tier_idx(rank);

        self.tiers.get_mut(tier_idx)?.get_mut_by_rank(rank)
    }

    fn expand(&mut self) {
        let curr_tier_size = self.tier_size();
        let new_tier_size = self.tier_size() << 1;

        for i in 0..(curr_tier_size / 2) {
            let second_tier = self.tiers.remove(i + 1);
            let first_tier = self
                .tiers
                .get_mut(i)
                .expect("tier does not exist at old index");

            first_tier.merge(second_tier);
        }

        for _ in 0..(new_tier_size - (curr_tier_size / 2)) {
            self.tiers.push(Tier::new(new_tier_size));
        }
    }

    fn try_contract(&mut self, num_entries: usize) {
        // only contract well below capacity to cull repeated alloc/free of memory upon reinsertion/redeletion
        if num_entries < self.capacity() / 8 {
            let new_tier_size = self.tier_size() >> 1;
            let _ = self.tiers.split_off(new_tier_size >> 1);

            let end_idx = new_tier_size;
            for i in (0..end_idx).step_by(2) {
                let old_tier = self.tiers.get_mut(i).expect("tier at index does not exist");
                let half_tier = old_tier.split_half();

                assert_eq!(half_tier.capacity(), new_tier_size);
                self.tiers.insert(i + 1, half_tier);
            }

            assert_eq!(self.tiers.len(), new_tier_size);
        }
    }

    pub fn insert(&mut self, rank: usize, elem: T) -> Result<usize, TieredVectorError<T>> {
        // @todo: why loop through every tier for every insert? find a different way to return this error
        let num_entries = self.len();
        if rank > num_entries {
            return Err(TieredVectorError::TieredVectorOutofBoundsInsertionError(
                rank, elem,
            ));
        }

        if num_entries == self.capacity() {
            self.expand();
        }

        let mut tier_idx = self.tier_idx(rank);
        let mut prev_popped = None;

        // pop-push phase
        if self
            .tiers
            .get(tier_idx)
            .expect("tier at index does not exist")
            .is_full()
        {
            for i in tier_idx..self.tiers.len() {
                let tier = self.tiers.get_mut(i).expect("tier at index does not exist");

                if tier.is_full() {
                    if let Ok(popped) = tier.pop_front() {
                        if let Some(prev_elem) = prev_popped {
                            tier.push_back(prev_elem).expect(
                                "tier did not have space despite prior call to `pop_front`",
                            );
                        }

                        prev_popped = Some(popped);
                    }
                } else {
                    if let Some(prev_elem) = prev_popped.take() {
                        tier.push_back(prev_elem)
                            .expect("tier did not have space despite prior call to `pop_front`");
                    }
                }
            }
        }

        // shift phase
        tier_idx = self.tier_idx(rank);
        let tier = self
            .tiers
            .get_mut(tier_idx)
            .expect("tier at index does not exist");
        tier.insert(rank, elem)
            .expect("could not insert into tier at rank");

        Ok(rank)
    }

    pub fn remove(&mut self, rank: usize) -> Result<T, TieredVectorError<T>> {
        let num_entries = self.len();
        if rank > num_entries {
            return Err(TieredVectorError::TieredVectorRankOutOfBoundsError(rank));
        }

        self.try_contract(num_entries);

        let tier_idx = self.tier_idx(rank);
        let mut prev_popped = None;

        // shift phase
        let target_tier = self
            .tiers
            .get_mut(tier_idx)
            .expect("tier at index does not exist");

        match target_tier.remove(rank) {
            Err(TierError::TierEmptyError) => Err(TieredVectorError::TieredVectorEmptyError),
            Err(TierError::TierRankOutOfBoundsError(r)) => {
                Err(TieredVectorError::TieredVectorRankOutOfBoundsError(r))
            }
            Err(_) => unreachable!(),

            Ok(removed) => {
                let last_tier_idx = self.tier_idx(num_entries);

                // pop-push phase
                for i in (tier_idx + 1..last_tier_idx + 1).rev() {
                    let tier = self.tiers.get_mut(i).expect("tier at index does not exist");

                    if let Ok(popped) = tier.pop_front() {
                        if let Some(prev_elem) = prev_popped {
                            tier.push_back(prev_elem).expect(
                                "tier did not have space despite prior call to `pop_front`",
                            );
                        }

                        prev_popped = Some(popped);
                    }
                }

                if let Some(popped) = prev_popped {
                    self.tiers[tier_idx]
                        .push_back(popped)
                        .expect("tier did not have space despite prior removal");
                }

                Ok(removed)
            }
        }
    }
}

impl<T: Clone + Debug> Debug for LinkedTieredVec<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_char('[')?;

        for i in 0..self.tiers.len() {
            let tier = &self.tiers[i];

            for j in 0..tier.buffer.len() {
                if let Some(elem) = tier.get(j) {
                    formatter.write_str(format!("{:?}", elem).as_str())?;
                } else {
                    formatter.write_str("_")?;
                }

                if j != tier.buffer.len() - 1 {
                    formatter.write_str(", ")?;
                }
            }

            if i != self.tiers.len() - 1 {
                formatter.write_str(", ")?;
            }
        }

        formatter.write_char(']')?;

        Ok(())
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
        assert_eq!(t.tier_size(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn with_minimum_capacity() {
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::with_minimum_capacity(4);
        assert_eq!(4, t.capacity());
        assert_eq!(2, t.tier_size());

        t = LinkedTieredVec::with_minimum_capacity(8);
        assert_eq!(16, t.capacity());
        assert_eq!(4, t.tier_size());

        t = LinkedTieredVec::with_minimum_capacity(128);
        assert_eq!(256, t.capacity());
        assert_eq!(16, t.tier_size());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);
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
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);

        for i in 0..size * size {
            assert!(t.insert(i, i * 2).is_ok());
        }
        assert_eq!(t.tier_size(), size);
        assert_eq!(t.len(), size * size);
        assert!(t.is_full());

        assert!(t.insert(size * size, (size * size) * 2).is_ok());
        assert_eq!(t.tier_size(), size * 2);
        assert_eq!(t.len(), (size * size) + 1);
        assert!(!t.is_full());

        for i in 0..((size * size) + 1) {
            let result = t.get_by_rank(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i * 2);
        }
    }

    #[test]
    fn remove_and_contract() {
        let size = 16;
        let mut t: LinkedTieredVec<usize> = LinkedTieredVec::new(size);
        assert_eq!(t.capacity(), size * size);

        for i in 0..size * size / 8 {
            assert!(t.insert(i, i).is_ok());
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

    #[test]
    fn contract() {}
}
