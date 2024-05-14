use std::{
    alloc::{Layout, LayoutError},
    mem::{size_of, MaybeUninit},
    ops::{Index, IndexMut},
};

#[repr(C)]
pub struct Tier<T> {
    head: usize,
    tail: usize,
    pub(crate) elements: [MaybeUninit<T>],
}

impl<T> Tier<T> {
    #[inline]
    pub fn size_of_metadata() -> usize {
        size_of::<usize>() * 2
    }

    #[inline]
    pub fn layout_for(tier_capacity: usize) -> Result<Layout, LayoutError> {
        let offsets = Layout::array::<usize>(2)?;
        let (layout, _) = offsets.extend(Layout::array::<MaybeUninit<T>>(tier_capacity)?)?;

        Ok(layout.pad_to_align())
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    pub const fn capacity(&self) -> usize {
        self.elements.len()
    }

    #[inline]
    const fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline]
    pub const fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    #[inline]
    pub fn head_forward(&mut self) {
        self.head = self.head.wrapping_add(1);
    }

    #[inline]
    fn head_backward(&mut self) {
        self.head = self.head.wrapping_sub(1);
    }

    #[inline]
    pub fn tail_forward_by(&mut self, extend_count: usize) {
        self.tail += extend_count;
    }

    #[inline]
    fn tail_forward(&mut self) {
        self.tail = self.tail.wrapping_add(1);
    }

    #[inline]
    pub fn tail_backward(&mut self) {
        self.tail = self.tail.wrapping_sub(1);
    }

    #[inline]
    pub fn clear_and_leak(&mut self) {
        self.head = 0;
        self.tail = 0;
    }

    #[inline]
    pub(crate) const fn masked_head(&self) -> usize {
        self.mask(self.head)
    }

    #[inline]
    pub(crate) const fn masked_tail(&self) -> usize {
        self.mask(self.tail)
    }

    #[inline]
    pub(crate) const fn masked_rank(&self, rank: usize) -> usize {
        self.mask(self.head.wrapping_add(rank))
    }

    const fn contains_masked_rank(&self, masked_rank: usize) -> bool {
        let masked_head = self.masked_head();
        let masked_tail = self.masked_tail();

        if self.is_full() {
            true
        } else if masked_head <= masked_tail {
            // standard case
            masked_rank >= masked_head && masked_rank < masked_tail
        } else {
            // wrapping case
            masked_rank >= masked_head || masked_rank < masked_tail
        }
    }

    pub const fn contains_rank(&self, rank: usize) -> bool {
        self.contains_masked_rank(self.masked_rank(rank))
    }

    fn get<'a>(&'a self, index: usize) -> Option<&'a T> {
        if !self.contains_masked_rank(index) {
            return None;
        }

        let elem = &self.elements[index];
        Some(unsafe { elem.assume_init_ref() })
    }

    fn get_mut<'a>(&'a mut self, index: usize) -> Option<&'a mut T> {
        if !self.contains_masked_rank(index) {
            return None;
        }

        let elem = &mut self.elements[index];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank<'a>(&'a self, rank: usize) -> Option<&'a T> {
        let masked_rank = self.masked_rank(rank);
        self.get(masked_rank)
    }

    pub fn get_by_rank_mut<'a>(&'a mut self, rank: usize) -> Option<&'a mut T> {
        let masked_rank = self.masked_rank(rank);
        self.get_mut(masked_rank)
    }

    pub fn rotate_reset(&mut self) {
        self.tail = self.len();

        let masked_head = self.masked_head();
        self.elements.rotate_left(masked_head);

        self.head = 0;
    }

    #[inline]
    fn set_element(&mut self, index: usize, elem: T) -> &mut T {
        self.elements[index].write(elem)
    }

    #[inline]
    fn take_element(&mut self, index: usize) -> T {
        let elem = &mut self.elements[index];
        unsafe { elem.assume_init_read() }
    }

    #[inline]
    fn replace_element(&mut self, index: usize, elem: T) -> T {
        let slot = &mut self.elements[index];
        unsafe { std::mem::replace(slot, MaybeUninit::new(elem)).assume_init() }
    }

    pub fn push_front(&mut self, elem: T) {
        assert!(!self.is_full());

        self.head_backward();

        let index = self.masked_head();
        self.set_element(index, elem);
    }

    pub fn push_back(&mut self, elem: T) {
        assert!(!self.is_full());

        let index = self.masked_tail();
        self.tail_forward();

        self.set_element(index, elem);
    }

    pub fn pop_front(&mut self) -> T {
        assert!(!self.is_empty());

        let index = self.masked_head();
        self.head_forward();

        self.take_element(index)
    }

    pub fn pop_back(&mut self) -> T {
        assert!(!self.is_empty());

        self.tail_backward();
        let index = self.masked_tail();

        self.take_element(index)
    }

    fn shift_to_head(&mut self, from: usize) {
        let mut cursor: Option<T> = None;
        let mut i = from;

        self.head_backward();
        let masked_head = self.masked_head();

        while i != masked_head {
            if let Some(curr_elem) = cursor {
                let elem = self.replace_element(i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = self.take_element(i);
                cursor = Some(elem);
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(curr_elem) = cursor {
            self.set_element(i, curr_elem);
        }
    }

    fn shift_to_tail(&mut self, from: usize) {
        let masked_tail = self.masked_tail();
        let mut cursor: Option<T> = None;
        let mut i = from;

        while i != masked_tail {
            if let Some(curr_elem) = cursor {
                cursor = Some(self.replace_element(i, curr_elem));
            } else {
                cursor = Some(self.take_element(i));
            }

            i = self.mask(i.wrapping_add(1));
        }

        if let Some(curr_elem) = cursor {
            self.set_element(i, curr_elem);
        }

        self.tail_forward();
    }

    pub fn insert(&mut self, rank: usize, elem: T) {
        assert!(!self.is_full());

        let masked_head = self.masked_head();
        let masked_tail = self.masked_tail();
        let masked_rank = self.masked_rank(rank);

        if masked_tail == masked_rank {
            self.push_back(elem);
        } else if masked_head == masked_rank {
            self.push_front(elem);
        } else {
            self.shift_to_tail(masked_rank);

            self.set_element(masked_rank, elem);
        }
    }

    fn close_gap(&mut self, gap_masked_idx: usize) {
        let mut cursor = None;

        self.tail_backward();
        let mut i = self.masked_tail();

        while i > gap_masked_idx {
            if let Some(elem) = cursor {
                cursor = Some(self.replace_element(i, elem));
            } else {
                cursor = Some(self.take_element(i));
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(elem) = cursor {
            self.set_element(i, elem);
        }
    }

    pub fn remove(&mut self, rank: usize) -> T {
        assert!(!self.is_empty());

        let masked_rank = self.masked_rank(rank);
        let elem = self.take_element(masked_rank);

        if masked_rank == self.masked_head() {
            self.head_forward();
        } else if masked_rank == self.masked_tail() {
            self.tail_backward();
        } else {
            self.close_gap(masked_rank);
        }

        return elem;
    }
}

impl<T> Index<usize> for Tier<T> {
    type Output = T;

    fn index(&self, rank: usize) -> &Self::Output {
        unsafe { self.elements[self.masked_rank(rank)].assume_init_ref() }
    }
}

impl<T> IndexMut<usize> for Tier<T> {
    fn index_mut(&mut self, rank: usize) -> &mut Self::Output {
        unsafe { self.elements[self.masked_rank(rank)].assume_init_mut() }
    }
}

#[cfg(test)]
mod tests {
    use crate::flat::tiered_vec::FlatTieredVec;
    use std::fmt::Debug;

    fn prepare_tiered_vec<T>(tier_capacity: usize) -> FlatTieredVec<T>
    where
        T: Debug,
    {
        FlatTieredVec::new(tier_capacity)
    }

    #[test]
    fn contains_rank() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        assert!(!tv.tier(0).contains_rank(0));
        assert!(!tv.tier(0).contains_rank(2));
        assert!(!tv.tier(0).contains_rank(4));

        tv.tier_mut(0).push_back(0);
        assert!(tv.tier(0).contains_rank(0));

        tv.tier_mut(0).push_back(1);
        assert!(tv.tier(0).contains_rank(0));
        assert!(tv.tier(0).contains_rank(1));

        tv.tier_mut(0).push_back(2);
        assert!(tv.tier(0).contains_rank(0));
        assert!(tv.tier(0).contains_rank(1));
        assert!(tv.tier(0).contains_rank(2));
        assert!(!tv.tier(0).contains_rank(3));
    }

    #[test]
    fn insert_at_rank_shift_tail() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [0, 1, 2, n]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);

        // [0, 1, 3, 2]
        tv.tier_mut(0).insert(2, 3);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 0);
        assert_eq!(*tv.tier(0).get(1).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 3);
        assert_eq!(*tv.tier(0).get(3).unwrap(), 2);
    }

    #[test]
    fn remove_at_rank_1() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [0, 1, 2, 3]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);
        tv.tier_mut(0).push_back(3);
        assert_eq!(tv.tier(0).masked_head(), 0);
        assert_eq!(tv.tier(0).masked_tail(), 0);

        // [0, 2, 3, _]
        tv.tier_mut(0).remove(1);
        assert_eq!(tv.tier(0).masked_head(), 0);
        assert_eq!(tv.tier(0).masked_tail(), 3);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 0);
        assert_eq!(*tv.tier(0).get(1).unwrap(), 2);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 3);
        assert!(tv.tier(0).get(3).is_none());
    }

    #[test]
    fn remove_at_rank_2() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [0, 1, 2, 3]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);
        tv.tier_mut(0).push_back(3);
        assert_eq!(tv.tier(0).masked_head(), 0);
        assert_eq!(tv.tier(0).masked_tail(), 0);

        // [_, 1, 2, 3]
        tv.tier_mut(0).remove(0);
        assert_eq!(tv.tier(0).masked_head(), 1);
        assert_eq!(tv.tier(0).masked_tail(), 0);
        assert!(tv.tier(0).get(0).is_none());
        assert_eq!(*tv.tier(0).get(1).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 2);
        assert_eq!(*tv.tier(0).get(3).unwrap(), 3);
    }

    #[test]
    fn shift_to_head_basic() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [0, 1, 2, n]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);

        // [1, 2, n, 0]
        tv.tier_mut(0).shift_to_head(2);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(1).unwrap(), 2);
        // assert_ne!(*tv.tier(0).get(2).unwrap(), 2);
        assert_eq!(*tv.tier(0).get(3).unwrap(), 0);
    }

    #[test]
    fn shift_to_head_data_middle_1() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [n, 1, 2, n]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);
        tv.tier_mut(0).pop_front();

        // [1, n, 2, n]
        tv.tier_mut(0).shift_to_head(1);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 1);
        // assert_ne!(*tv.tier(0).get(1).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 2);
        assert!(tv.tier(0).get(3).is_none());
    }

    #[test]
    fn shift_to_head_data_middle_2() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [n, 1, 2, n]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);
        tv.tier_mut(0).pop_front();

        // [1, 2, n, n]
        tv.tier_mut(0).shift_to_head(1);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 2);
        assert_ne!(*tv.tier(0).get(1).unwrap(), 2);
        assert!(tv.tier(0).get(3).is_none());
    }

    #[test]
    fn shift_to_tail_nonwrapping() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [0, 1, 2, n]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);

        // [0, n, 1, 2]
        tv.tier_mut(0).shift_to_tail(1);
        assert_eq!(*tv.tier(0).get(0).unwrap(), 0);
        // assert_ne!(*tv.tier(0).get(1).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(3).unwrap(), 2);
    }

    #[test]
    fn shift_to_tail_wrapping() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        // [3, n, 1, 2]
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(0);
        tv.tier_mut(0).push_back(1);
        tv.tier_mut(0).push_back(2);
        tv.tier_mut(0).pop_front();
        tv.tier_mut(0).pop_front();
        tv.tier_mut(0).push_back(3);

        // [n, 3, 1, 2]
        tv.tier_mut(0).shift_to_tail(0);
        // assert_ne!(*tv.tier(0).get(0).unwrap(), 3);
        assert_eq!(*tv.tier(0).get(1).unwrap(), 3);
        assert_eq!(*tv.tier(0).get(2).unwrap(), 1);
        assert_eq!(*tv.tier(0).get(3).unwrap(), 2);
    }

    #[test]
    fn push_and_pop() {
        let mut tv: FlatTieredVec<usize> = prepare_tiered_vec(4);

        assert!(tv.tier(0).is_empty());
        assert!(!tv.tier(0).is_full());
        assert_eq!(tv.tier(0).len(), 0);
        assert_eq!(tv.tier(0).capacity(), 4);

        // [n, n, n, 0]
        tv.tier_mut(0).push_front(0);
        assert_eq!(tv.tier(0).len(), 1);
        assert_eq!(tv.tier(0).masked_head(), 3);
        assert_eq!(tv.tier(0).masked_tail(), 0);
        assert!(tv.tier(0).get(3).is_some());
        assert_eq!(*tv.tier(0).get(3).unwrap(), 0);
        assert_eq!(*tv.tier(0).get_by_rank(0).unwrap(), 0);

        assert!(!tv.tier(0).contains_masked_rank(0));
        assert!(!tv.tier(0).contains_masked_rank(1));
        assert!(!tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        // [1, n, n, 0]
        tv.tier_mut(0).push_back(1);
        assert_eq!(tv.tier(0).len(), 2);
        assert_eq!(tv.tier(0).masked_head(), 3);
        assert_eq!(tv.tier(0).masked_tail(), 1);
        assert!(tv.tier(0).get(0).is_some());
        assert_eq!(*tv.tier(0).get(0).unwrap(), 1);
        assert_eq!(*tv.tier(0).get_by_rank(1).unwrap(), 1);

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(!tv.tier(0).contains_masked_rank(1));
        assert!(!tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        // [1, n, 2, 0]
        tv.tier_mut(0).push_front(2);
        assert_eq!(tv.tier(0).len(), 3);
        assert_eq!(tv.tier(0).masked_head(), 2);
        assert_eq!(tv.tier(0).masked_tail(), 1);
        assert!(tv.tier(0).get(2).is_some());
        assert_eq!(*tv.tier(0).get(2).unwrap(), 2);
        assert_eq!(*tv.tier(0).get_by_rank(0).unwrap(), 2);

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(!tv.tier(0).contains_masked_rank(1));
        assert!(tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        // [1, 3, 2, 0]
        tv.tier_mut(0).push_back(3);
        assert_eq!(tv.tier(0).len(), 4);
        assert_eq!(tv.tier(0).masked_head(), 2);
        assert_eq!(tv.tier(0).masked_tail(), 2);
        assert!(tv.tier(0).get(1).is_some());
        assert_eq!(*tv.tier(0).get(1).unwrap(), 3);
        assert_eq!(*tv.tier(0).get_by_rank(3).unwrap(), 3);

        assert!(!tv.tier(0).is_empty());
        assert!(tv.tier(0).is_full());

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(tv.tier(0).contains_masked_rank(1));
        assert!(tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        assert_eq!(tv.tier(0).masked_head(), 2);
        assert_eq!(tv.tier(0).masked_tail(), 2);

        // [1, 3, n, 0]
        let mut v = tv.tier_mut(0).pop_front();
        assert_eq!(v, 2);
        assert!(!tv.tier(0).is_empty());
        assert!(!tv.tier(0).is_full());
        assert_eq!(tv.tier(0).len(), 3);
        assert_eq!(tv.tier(0).masked_head(), 3);
        assert_eq!(tv.tier(0).masked_tail(), 2);
        assert!(tv.tier(0).get(2).is_none());

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(tv.tier(0).contains_masked_rank(1));
        assert!(!tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        // [1, n, n, 0]
        v = tv.tier_mut(0).pop_back();
        assert_eq!(v, 3);
        assert!(!tv.tier(0).is_empty());
        assert!(!tv.tier(0).is_full());
        assert_eq!(tv.tier(0).len(), 2);
        assert_eq!(tv.tier(0).masked_head(), 3);
        assert_eq!(tv.tier(0).masked_tail(), 1);
        assert!(tv.tier(0).get(1).is_none());

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(!tv.tier(0).contains_masked_rank(1));
        assert!(!tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));

        // [1, n, 4, 0]
        tv.tier_mut(0).push_front(4);
        assert_eq!(tv.tier(0).len(), 3);
        assert_eq!(tv.tier(0).masked_head(), 2);
        assert_eq!(tv.tier(0).masked_tail(), 1);
        assert!(tv.tier(0).get(2).is_some());
        assert_eq!(*tv.tier(0).get(2).unwrap(), 4);
        assert_eq!(*tv.tier(0).get_by_rank(0).unwrap(), 4);

        assert!(tv.tier(0).contains_masked_rank(0));
        assert!(!tv.tier(0).contains_masked_rank(1));
        assert!(tv.tier(0).contains_masked_rank(2));
        assert!(tv.tier(0).contains_masked_rank(3));
    }
}
