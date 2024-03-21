use std::fmt::Debug;

use super::tier::Tier;

pub type TieredVecIndex = usize;

pub struct TieredVec<T> {
    pub(crate) tier_size: usize, // todo: tier_size is implied with number of tiers
    pub(crate) tiers: Vec<Tier<T>>,
}

impl<T> TieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub(crate) fn new(initial_capacity: usize) -> Self {
        assert!(initial_capacity.is_power_of_two());

        Self {
            tier_size: initial_capacity,
            tiers: Vec::with_capacity(initial_capacity),
        }
    }

    pub(crate) const fn tier_index(&self, idx: TieredVecIndex) -> usize {
        idx / self.tier_size
    }

    pub fn capacity(&self) -> usize {
        self.tier_size * self.tiers.len()
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
        self.tiers.get(self.tier_index(rank))?.get_by_rank(rank)
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        let tier_idx = self.tier_index(rank);

        self.tiers.get_mut(tier_idx)?.get_mut_by_rank(rank)
    }

    fn expand(&mut self) {
        let new_tier_size = self.tier_size * 2;
        let mut new_tiers = Vec::with_capacity(new_tier_size);

        for i in 0..new_tier_size {
            let mut new_tier = Tier::new(new_tier_size);
            let old_tier_idx = i * 2;

            for j in old_tier_idx..old_tier_idx + 2 {
                let old_tier = self
                    .tiers
                    .get_mut(j)
                    .expect("tier does not exist at old index");

                // todo: can be optimized if T is Copy
                while let Ok(elem) = old_tier.pop_front() {
                    new_tier
                        .push_back(elem)
                        .expect("new tier does not have enough space");
                }
            }

            new_tiers.push(new_tier);
        }

        self.tier_size = new_tier_size;
        self.tiers = new_tiers;
    }

    fn contract(&mut self) {
        let new_tier_size = self.tier_size / 2;
        let mut new_tiers = Vec::with_capacity(new_tier_size);

        for i in 0..self.tier_size {
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

        self.tier_size = new_tier_size;
        self.tiers = new_tiers;
    }

    pub fn insert(&mut self, rank: usize, elem: T) {
        if self.is_full() {
            self.expand();
        }

        let tier_idx = self.tier_index(rank);
        let last_tier_idx = self.tier_index(self.len());
        let mut prev_popped = None;

        // pop-push phase
        for i in tier_idx..last_tier_idx + 1 {
            let tier = self.tiers.get_mut(i).expect("tier at index does not exist");

            if let Ok(popped) = tier.pop_back() {
                if let Some(prev_elem) = prev_popped {
                    tier.push_front(prev_elem)
                        .expect("tier did not have space despite prior call to `pop_back`");
                }

                prev_popped = Some(popped);
            }
        }

        // shift phase
        let tier = self
            .tiers
            .get_mut(tier_idx)
            .expect("tier at index does not exist");
        tier.insert_at_rank(rank, elem)
            .expect("could not insert into tier at rank");
    }

    pub fn remove(&mut self, rank: usize) -> Option<T> {
        let tier_idx = self.tier_index(rank);
        let last_tier_idx = self.tier_index(self.len());
        let mut prev_popped = None;

        // shift phase
        if let Some(tier) = self.tiers.get_mut(tier_idx) {
            if let Ok(removed) = tier.remove_at_rank(rank) {
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
