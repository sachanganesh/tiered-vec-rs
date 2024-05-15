use std::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
};

pub struct Tier<T> {
    head: usize,
    tail: usize,
    elements: Vec<MaybeUninit<T>>,
}

impl<T> Tier<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two());

        let mut vec = Vec::with_capacity(capacity);
        unsafe {
            vec.set_len(vec.capacity());
        }

        Self {
            elements: vec,
            head: 0,
            tail: 0,
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.elements.len()
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    #[inline]
    fn head_forward(&mut self) {
        self.head = self.head.wrapping_add(1);
    }

    #[inline]
    fn head_backward(&mut self) {
        self.head = self.head.wrapping_sub(1);
    }

    #[inline]
    fn tail_forward(&mut self) {
        self.tail = self.tail.wrapping_add(1);
    }

    #[inline]
    fn tail_backward(&mut self) {
        self.tail = self.tail.wrapping_sub(1);
    }

    #[inline]
    pub(crate) fn masked_head(&self) -> usize {
        self.mask(self.head)
    }

    #[inline]
    pub(crate) fn masked_tail(&self) -> usize {
        self.mask(self.tail)
    }

    #[inline]
    pub(crate) fn masked_rank(&self, rank: usize) -> usize {
        self.mask(self.head.wrapping_add(rank))
    }

    fn contains_masked_rank(&self, masked_rank: usize) -> bool {
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

    pub fn contains_rank(&self, rank: usize) -> bool {
        self.contains_masked_rank(self.masked_rank(rank))
    }

    pub(crate) fn get(&self, idx: usize) -> Option<&T> {
        if !self.contains_masked_rank(idx) {
            return None;
        }

        let elem = &self.elements[idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub(crate) fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        if !self.contains_masked_rank(idx) {
            return None;
        }

        let elem = &mut self.elements[idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        self.get(self.masked_rank(rank))
    }

    pub fn get_by_rank_mut(&mut self, rank: usize) -> Option<&mut T> {
        self.get_mut(self.masked_rank(rank))
    }

    pub fn rotate_reset(&mut self) {
        self.tail = self.len();

        let masked_head = self.masked_head();
        self.elements.rotate_left(masked_head);

        self.head = 0;
    }

    #[inline]
    fn set_element(&mut self, masked_idx: usize, elem: T) -> &mut T {
        self.elements[masked_idx].write(elem)
    }

    #[inline]
    fn take_element(&mut self, masked_idx: usize) -> T {
        let elem = &mut self.elements[masked_idx];
        unsafe { elem.assume_init_read() }
    }

    #[inline]
    fn replace_element(&mut self, masked_idx: usize, elem: T) -> T {
        let slot = &mut self.elements[masked_idx];
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

    pub fn pop_push_front(&mut self, elem: T) -> T {
        assert!(self.is_full());

        self.head_backward();
        self.tail_backward();
        let index = self.masked_head();

        self.replace_element(index, elem)
    }

    pub fn pop_push_back(&mut self, elem: T) -> T {
        assert!(self.is_full());

        let index = self.masked_tail();
        self.head_forward();
        self.tail_forward();

        self.replace_element(index, elem)
    }

    fn shift_to_head(&mut self, from: usize) {
        let mut cursor: Option<T> = None;
        let mut i = from;

        self.head_backward();

        while i != self.masked_head() {
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
                let elem = self.replace_element(i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = self.take_element(i);
                cursor = Some(elem);
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

    pub fn merge(&mut self, mut other: Tier<T>) {
        self.elements.reserve_exact(other.capacity());
        unsafe {
            self.elements.set_len(self.elements.capacity());
        }

        for _ in 0..other.len() {
            self.push_back(other.pop_front());
        }
    }

    pub fn split_half(&mut self) -> Tier<T> {
        self.rotate_reset();
        let count = self.len();
        let new_capacity = self.capacity() / 2;

        let new_buffer = self.elements.split_off(new_capacity);
        let remaining_tail = count.saturating_sub(new_capacity);
        self.tail = count.saturating_sub(remaining_tail);

        let new_t = Tier {
            elements: new_buffer,
            head: 0,
            tail: remaining_tail,
        };

        return new_t;
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
        let index = self.masked_rank(rank);
        unsafe { self.elements[index].assume_init_mut() }
    }
}

impl<T> Clone for Tier<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        let mut buffer: Vec<MaybeUninit<T>> = Vec::with_capacity(self.elements.capacity());
        unsafe {
            buffer.set_len(buffer.capacity());
        }

        let mut i = self.head;

        while i != self.tail {
            let idx = self.mask(i);

            buffer[idx] = MaybeUninit::new(
                self.get(idx)
                    .expect("tried to retrieve element from valid index")
                    .clone(),
            );

            i += 1;
        }

        Self {
            elements: buffer,
            head: self.head,
            tail: self.tail,
        }
    }
}

impl<T> Drop for Tier<T> {
    fn drop(&mut self) {
        for _ in 0..self.len() {
            self.pop_back();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn error_on_wrong_tier_size() {
        let _t: Tier<usize> = Tier::new(5);
    }

    #[test]
    fn no_error_on_correct_tier_size() {
        let t: Tier<usize> = Tier::new(4);
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), 4);
    }

    #[test]
    fn contains_rank() {
        let mut t: Tier<usize> = Tier::new(4);
        assert!(!t.contains_rank(0));
        assert!(!t.contains_rank(2));
        assert!(!t.contains_rank(4));

        t.push_back(0);
        assert!(t.contains_rank(0));
        t.push_back(1);
        assert!(t.contains_rank(0));
        assert!(t.contains_rank(1));
        t.push_back(2);
        assert!(t.contains_rank(0));
        assert!(t.contains_rank(1));
        assert!(t.contains_rank(2));
        assert!(!t.contains_rank(3));
    }

    #[test]
    fn insert_at_rank_shift_tail() {
        let mut t = Tier::new(4);

        // [0, 1, 2, n]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);

        // [0, 1, 3, 2]
        t.insert(2, 3);
        assert_eq!(*t.get(0).unwrap(), 0);
        assert_eq!(*t.get(1).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 3);
        assert_eq!(*t.get(3).unwrap(), 2);
    }

    #[test]
    fn remove_at_rank_1() {
        let mut t = Tier::new(4);

        // [0, 1, 2, 3]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);
        t.push_back(3);
        assert_eq!(t.masked_head(), 0);
        assert_eq!(t.masked_tail(), 0);

        // [0, 2, 3, _]
        t.remove(1);
        assert_eq!(t.masked_head(), 0);
        assert_eq!(t.masked_tail(), 3);
        assert_eq!(*t.get(0).unwrap(), 0);
        assert_eq!(*t.get(1).unwrap(), 2);
        assert_eq!(*t.get(2).unwrap(), 3);
        assert!(t.get(3).is_none());
    }

    #[test]
    fn remove_at_rank_2() {
        let mut t = Tier::new(4);

        // [0, 1, 2, 3]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);
        t.push_back(3);
        assert_eq!(t.masked_head(), 0);
        assert_eq!(t.masked_tail(), 0);

        // [_, 1, 2, 3]
        t.remove(0);
        assert_eq!(t.masked_head(), 1);
        assert_eq!(t.masked_tail(), 0);
        assert!(t.get(0).is_none());
        assert_eq!(*t.get(1).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 2);
        assert_eq!(*t.get(3).unwrap(), 3);
    }

    #[test]
    fn shift_to_head_basic() {
        let mut t = Tier::new(4);

        // [0, 1, 2, n]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);

        // [1, 2, n, 0]
        t.shift_to_head(2);
        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(*t.get(1).unwrap(), 2);
        // assert_ne!(*t.get(2).unwrap(), 2);
        assert_eq!(*t.get(3).unwrap(), 0);
    }

    #[test]
    fn shift_to_head_data_middle_1() {
        let mut t = Tier::new(4);

        // [n, 1, 2, n]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);
        t.pop_front();

        // [1, n, 2, n]
        t.shift_to_head(1);
        assert_eq!(*t.get(0).unwrap(), 1);
        // assert_ne!(*t.get(1).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 2);
        assert!(t.get(3).is_none());
    }

    #[test]
    fn shift_to_head_data_middle_2() {
        let mut t = Tier::new(4);

        // [n, 1, 2, n]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);
        t.pop_front();

        // [1, 2, n, n]
        t.shift_to_head(1);
        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 2);
        assert_ne!(*t.get(1).unwrap(), 2);
        assert!(t.get(3).is_none());
    }

    #[test]
    fn shift_to_tail_nonwrapping() {
        let mut t = Tier::new(4);

        // [0, 1, 2, n]
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);

        // [0, n, 1, 2]
        t.shift_to_tail(1);
        assert_eq!(*t.get(0).unwrap(), 0);
        // assert_ne!(*t.get(1).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 1);
        assert_eq!(*t.get(3).unwrap(), 2);
    }

    #[test]
    fn shift_to_tail_wrapping() {
        let mut t = Tier::new(4);

        // [3, n, 1, 2]
        t.push_back(0);
        t.push_back(0);
        t.push_back(1);
        t.push_back(2);
        t.pop_front();
        t.pop_front();
        t.push_back(3);

        // [n, 3, 1, 2]
        t.shift_to_tail(0);
        // assert_ne!(*t.get(0).unwrap(), 3);
        assert_eq!(*t.get(1).unwrap(), 3);
        assert_eq!(*t.get(2).unwrap(), 1);
        assert_eq!(*t.get(3).unwrap(), 2);
    }

    #[test]
    fn push_and_pop() {
        let mut t: Tier<usize> = Tier::new(4);
        assert!(t.is_empty());
        assert!(!t.is_full());
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), 4);

        // [n, n, n, 0]
        t.push_front(0);
        assert_eq!(t.len(), 1);
        assert_eq!(t.masked_head(), 3);
        assert_eq!(t.masked_tail(), 0);
        assert!(t.get(3).is_some());
        assert_eq!(*t.get(3).unwrap(), 0);
        assert_eq!(*t.get_by_rank(0).unwrap(), 0);

        assert!(!t.contains_masked_rank(0));
        assert!(!t.contains_masked_rank(1));
        assert!(!t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // [1, n, n, 0]
        t.push_back(1);
        assert_eq!(t.len(), 2);
        assert_eq!(t.masked_head(), 3);
        assert_eq!(t.masked_tail(), 1);
        assert!(t.get(0).is_some());
        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(*t.get_by_rank(1).unwrap(), 1);

        assert!(t.contains_masked_rank(0));
        assert!(!t.contains_masked_rank(1));
        assert!(!t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // [1, n, 2, 0]
        t.push_front(2);
        assert_eq!(t.len(), 3);
        assert_eq!(t.masked_head(), 2);
        assert_eq!(t.masked_tail(), 1);
        assert!(t.get(2).is_some());
        assert_eq!(*t.get(2).unwrap(), 2);
        assert_eq!(*t.get_by_rank(0).unwrap(), 2);

        assert!(t.contains_masked_rank(0));
        assert!(!t.contains_masked_rank(1));
        assert!(t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // [1, 3, 2, 0]
        t.push_back(3);
        assert_eq!(t.len(), 4);
        assert_eq!(t.masked_head(), 2);
        assert_eq!(t.masked_tail(), 2);
        assert!(t.get(1).is_some());
        assert_eq!(*t.get(1).unwrap(), 3);
        assert_eq!(*t.get_by_rank(3).unwrap(), 3);

        assert!(!t.is_empty());
        assert!(t.is_full());

        assert!(t.contains_masked_rank(0));
        assert!(t.contains_masked_rank(1));
        assert!(t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // t.push_back(4); // err
        assert_eq!(t.masked_head(), 2);
        assert_eq!(t.masked_tail(), 2);

        // [1, 3, n, 0]
        let mut v = t.pop_front();
        assert_eq!(v, 2);
        assert!(!t.is_empty());
        assert!(!t.is_full());
        assert_eq!(t.len(), 3);
        assert_eq!(t.masked_head(), 3);
        assert_eq!(t.masked_tail(), 2);
        assert!(t.get(2).is_none());

        assert!(t.contains_masked_rank(0));
        assert!(t.contains_masked_rank(1));
        assert!(!t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // [1, n, n, 0]
        v = t.pop_back();
        assert_eq!(v, 3);
        assert!(!t.is_empty());
        assert!(!t.is_full());
        assert_eq!(t.len(), 2);
        assert_eq!(t.masked_head(), 3);
        assert_eq!(t.masked_tail(), 1);
        assert!(t.get(1).is_none());

        assert!(t.contains_masked_rank(0));
        assert!(!t.contains_masked_rank(1));
        assert!(!t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));

        // [1, n, 4, 0]
        t.push_front(4);
        assert_eq!(t.len(), 3);
        assert_eq!(t.masked_head(), 2);
        assert_eq!(t.masked_tail(), 1);
        assert!(t.get(2).is_some());
        assert_eq!(*t.get(2).unwrap(), 4);
        assert_eq!(*t.get_by_rank(0).unwrap(), 4);

        assert!(t.contains_masked_rank(0));
        assert!(!t.contains_masked_rank(1));
        assert!(t.contains_masked_rank(2));
        assert!(t.contains_masked_rank(3));
    }
}
