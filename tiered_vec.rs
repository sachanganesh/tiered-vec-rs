use std::fmt::Debug;

use super::tier::Tier;

pub struct TieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub(crate) tiers: Vec<Tier<T>>,
}

impl<T> TieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
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

    #[inline(always)]
    fn tier_size(&self) -> usize {
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
        self.len() == self.capacity()
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

    fn contract(&mut self) {
        let new_tier_size = self.tier_size() >> 1;
        let mut new_tiers = Vec::with_capacity(new_tier_size);

        for i in 0..self.tier_size() {
            let old_tier = self
                .tiers
                .get_mut(i)
                .expect("tier does not exist at old index");

            let mut new_tier_half = Tier::new(new_tier_size);
            let mut new_tier_rest = Tier::new(new_tier_size);

            let mut j = 0;
            while j < new_tier_size && j < old_tier.len() {
                // todo: can be optimized if T is Copy
                if let Ok(elem) = old_tier.pop_front() {
                    new_tier_half
                        .push_back(elem)
                        .expect("new tier does not have enough space");
                }

                j += 1;
            }

            j = 0;
            while j < new_tier_size && j < old_tier.len() {
                // todo: can be optimized if T is Copy
                if let Ok(elem) = old_tier.pop_front() {
                    new_tier_rest
                        .push_back(elem)
                        .expect("new tier does not have enough space");
                }

                j += 1;
            }

            new_tiers.push(new_tier_half);
            new_tiers.push(new_tier_rest);
        }

        self.tiers = new_tiers;
    }

    pub fn insert(&mut self, rank: usize, elem: T) {
        if self.is_full() {
            self.expand();
        }

        let mut tier_idx = self.tier_idx(rank);
        let last_tier_idx = self.tier_idx(self.len());
        let mut prev_popped = None;

        // pop-push phase
        if self
            .tiers
            .get(tier_idx)
            .expect("tier at index does not exist")
            .is_full()
        {
            for i in tier_idx..last_tier_idx + 1 {
                let tier = self.tiers.get_mut(i).expect("tier at index does not exist");

                if let Ok(popped) = tier.pop_front() {
                    if let Some(prev_elem) = prev_popped {
                        tier.push_back(prev_elem)
                            .expect("tier did not have space despite prior call to `pop_back`");
                    }

                    prev_popped = Some(popped);
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
    }

    pub fn remove(&mut self, rank: usize) -> Option<T> {
        let tier_idx = self.tier_idx(rank);
        let last_tier_idx = self.tier_idx(self.len());
        let mut prev_popped = None;

        // shift phase
        if let Some(tier) = self.tiers.get_mut(tier_idx) {
            if let Ok(removed) = tier.remove(rank) {
                // pop-push phase
                for i in (tier_idx..last_tier_idx + 1).rev() {
                    let tier = self.tiers.get_mut(i).expect("tier at index does not exist");

                    if let Ok(popped) = tier.pop_back() {
                        if let Some(prev_elem) = prev_popped {
                            tier.push_front(prev_elem)
                                .expect("tier did not have space despite prior call to `pop_back`");
                        }

                        prev_popped = Some(popped);
                    }
                }

                // potentially contract

                return Some(removed);
            }
        }

        return None;
    }
}

#[cfg(test)]
mod tests {
    use crate::cache_conscious::tiered_vec::*;

    #[test]
    #[should_panic]
    fn error_on_non_power_of_two_size() {
        let _t: TieredVec<usize> = TieredVec::new(5);
    }

    #[test]
    #[should_panic]
    fn error_on_small_size() {
        let _t: TieredVec<usize> = TieredVec::new(1);
    }

    #[test]
    fn no_error_on_correct_size() {
        let size = 4;
        let t: TieredVec<usize> = TieredVec::new(size);
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), size * size);
        assert_eq!(t.tier_size(), size);
        assert!(t.is_empty());
        assert!(!t.is_full());
    }

    #[test]
    fn insert() {
        let size = 4;
        let mut t: TieredVec<usize> = TieredVec::new(size);
        assert_eq!(t.tier_size(), size);

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
        let mut t: TieredVec<usize> = TieredVec::new(size);

        for i in 0..size * size {
            t.insert(i, i * 2);
        }
        assert_eq!(t.tier_size(), size);
        assert_eq!(t.len(), size * size);
        assert!(t.is_full());

        t.insert(size * size, (size * size) * 2);
        assert_eq!(t.tier_size(), size * 2);
        assert_eq!(t.len(), (size * size) + 1);
        assert!(!t.is_full());

        for i in 0..((size * size) + 1) {
            let result = t.get_by_rank(i);
            assert!(result.is_some());
            assert_eq!(*result.unwrap(), i * 2);
        }
    }
}
