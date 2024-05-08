use std::{
    fmt::{Debug, Write},
    mem::MaybeUninit,
    ops::{Deref, DerefMut, Range},
};

use super::tier_error::TierError;

#[repr(transparent)]
#[derive(Clone)]
pub struct Tier<T>
where
    T: Clone + Debug,
{
    inner: RawTier<T>,
}

impl<T> Tier<T>
where
    T: Clone + Debug,
{
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: RawTier::new(capacity),
        }
    }
}

impl<T: Clone + Debug> Deref for Tier<T> {
    type Target = RawTier<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: Clone + Debug> DerefMut for Tier<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: Clone + Debug> Debug for Tier<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.inner.fmt(f)
    }
}

pub struct RawTier<T>
where
    T: Clone + Debug,
{
    pub(crate) buffer: Vec<MaybeUninit<T>>,
    pub(crate) head: usize,
    pub(crate) tail: usize,
}

impl<T> RawTier<T>
where
    T: Clone + Debug,
{
    pub(crate) fn new(capacity: usize) -> Self {
        assert!(capacity.is_power_of_two());

        let mut vec = Vec::with_capacity(capacity);
        unsafe {
            vec.set_len(vec.capacity());
        }

        Self {
            buffer: vec,
            head: 0,
            tail: 0,
        }
    }

    #[inline]
    fn mask(&self, val: usize) -> usize {
        val & (self.buffer.len() - 1)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
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
    pub const fn max_rank(&self) -> usize {
        self.len() - 1
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
        if self.is_empty() {
            false
        } else if self.is_full() {
            true
        } else if masked_head < masked_tail {
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

        let elem = &self.buffer[idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub(crate) fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        if !self.contains_masked_rank(idx) {
            return None;
        }

        let elem = &mut self.buffer[idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn get_by_rank(&self, rank: usize) -> Option<&T> {
        self.get(self.masked_rank(rank))
    }

    pub fn get_mut_by_rank(&mut self, rank: usize) -> Option<&mut T> {
        self.get_mut(self.masked_rank(rank))
    }

    pub fn get_range_by_rank(&self, range: Range<usize>) -> Option<Vec<&T>> {
        todo!()
    }

    pub fn get_mut_range_by_rank(&self, range: Range<usize>) -> Option<Vec<&mut T>> {
        todo!()
    }

    pub fn rotate_reset(&mut self) {
        self.tail = self.len();

        let masked_head = self.masked_head();
        self.buffer.rotate_left(masked_head);

        self.head = 0;
    }

    fn set(&mut self, masked_idx: usize, elem: T) -> &mut T {
        self.buffer[masked_idx].write(elem)
    }

    fn take(&mut self, masked_idx: usize) -> T {
        let slot = &mut self.buffer[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::uninit()).assume_init() }
    }

    fn replace(&mut self, masked_idx: usize, elem: T) -> T {
        let slot = &mut self.buffer[masked_idx];
        unsafe { std::mem::replace(slot, MaybeUninit::new(elem)).assume_init() }
    }

    pub fn push_front(&mut self, elem: T) -> Result<usize, TierError<T>> {
        if !self.is_full() {
            self.head_backward();
            let idx = self.masked_head();

            self.set(idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn push_back(&mut self, elem: T) -> Result<usize, TierError<T>> {
        if !self.is_full() {
            let idx = self.masked_tail();
            self.tail_forward();

            self.set(idx, elem);
            Ok(idx)
        } else {
            Err(TierError::TierFullInsertionError(elem).into())
        }
    }

    pub fn pop_front(&mut self) -> Result<T, TierError<T>> {
        if !self.is_empty() {
            let idx = self.masked_head();
            self.head_forward();

            Ok(self.take(idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    pub fn pop_back(&mut self) -> Result<T, TierError<T>> {
        if !self.is_empty() {
            self.tail_backward();
            let idx = self.masked_tail();

            Ok(self.take(idx))
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    fn shift_to_head(&mut self, from: usize) {
        let mut cursor: Option<T> = None;
        let mut i = from;

        self.head_backward();

        while i != self.masked_head() {
            if let Some(curr_elem) = cursor {
                let elem = self.replace(i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = self.take(i);
                cursor = Some(elem);
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(curr_elem) = cursor {
            self.set(i, curr_elem);
        }
    }

    fn shift_to_tail(&mut self, from: usize) {
        let masked_tail = self.masked_tail();
        let mut cursor: Option<T> = None;
        let mut i = from;

        while i < masked_tail {
            if let Some(curr_elem) = cursor {
                let elem = self.replace(i, curr_elem);
                cursor = Some(elem);
            } else {
                let elem = self.take(i);
                cursor = Some(elem);
            }

            i = self.mask(i.wrapping_add(1));
        }

        if let Some(curr_elem) = cursor {
            self.set(i, curr_elem);
            self.tail_forward();
        }
    }

    pub fn insert(&mut self, rank: usize, elem: T) -> Result<usize, TierError<T>> {
        if self.is_full() {
            return Err(TierError::TierFullInsertionError(elem));
        }

        let masked_head = self.masked_head();
        let masked_tail = self.masked_tail();
        let masked_rank = self.masked_rank(rank);

        if masked_tail == masked_rank {
            self.push_back(elem)
        } else if self.contains_masked_rank(masked_rank) {
            if masked_head == masked_rank {
                self.push_front(elem)
            } else {
                let head_delta = masked_rank.abs_diff(masked_head);
                let tail_delta = masked_rank.abs_diff(masked_tail);

                if head_delta <= tail_delta {
                    self.shift_to_head(masked_rank);
                } else {
                    self.shift_to_tail(masked_rank);
                }

                self.set(masked_rank, elem);

                Ok(masked_rank)
            }
        } else {
            Err(TierError::TierDisconnectedEntryInsertionError(rank, elem))
        }
    }

    fn close_gap(&mut self, gap_masked_idx: usize) {
        let mut cursor = None;

        self.tail_backward();
        let mut i = self.masked_tail();

        while i > gap_masked_idx {
            if let Some(elem) = cursor {
                cursor = Some(self.replace(i, elem));
            } else {
                cursor = Some(self.take(i));
            }

            i = self.mask(i.wrapping_sub(1));
        }

        if let Some(elem) = cursor {
            self.set(i, elem);
        }
    }

    pub fn remove(&mut self, rank: usize) -> Result<T, TierError<T>> {
        if self.is_empty() {
            return Err(TierError::TierEmptyError);
        }

        let masked_rank = self.masked_rank(rank);

        if self.contains_masked_rank(masked_rank) {
            let elem = self.take(masked_rank);

            if masked_rank == self.masked_head() {
                self.head_forward();
            } else if masked_rank == self.masked_tail() {
                self.tail_backward();
            } else {
                self.close_gap(masked_rank);
            }

            Ok(elem)
        } else {
            Err(TierError::TierRankOutOfBoundsError(rank))
        }
    }

    pub fn merge(&mut self, mut other: Tier<T>) {
        self.rotate_reset();
        self.buffer.reserve_exact(other.capacity());
        unsafe {
            self.buffer.set_len(self.buffer.capacity());
        }

        while let Ok(elem) = other.pop_front() {
            self.push_back(elem)
                .expect("resized tier could not merge element due to size");
        }
    }

    pub fn merge_copy(&mut self, mut other: Tier<T>)
    where
        T: Copy,
    {
        self.rotate_reset();
        other.rotate_reset();

        self.buffer.reserve_exact(other.capacity());

        todo!()
    }

    pub fn split_half(&mut self) -> Tier<T> {
        self.rotate_reset();
        let count = self.len();
        let new_capacity = self.capacity() / 2;

        let new_buffer = self.buffer.split_off(new_capacity);
        let remaining_tail = count.saturating_sub(new_capacity);
        self.tail = count.saturating_sub(remaining_tail);

        let new_t = Tier {
            inner: RawTier {
                buffer: new_buffer,
                head: 0,
                tail: remaining_tail,
            },
        };

        return new_t;
    }
}

impl<T: Clone + Debug> Clone for RawTier<T> {
    fn clone(&self) -> Self {
        let mut buffer: Vec<MaybeUninit<T>> = Vec::with_capacity(self.buffer.capacity());
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
            buffer,
            head: self.head,
            tail: self.tail,
        }
    }
}

impl<T: Clone + Debug> Drop for RawTier<T> {
    fn drop(&mut self) {
        if !self.is_empty() {
            let mut i = self.masked_head();
            let masked_tail = self.masked_tail();
            while i != masked_tail {
                unsafe {
                    self.buffer[i].assume_init_drop();
                }

                i = self.mask(i.wrapping_add(1));
            }
        }
    }
}

impl<T: Clone + Debug> Debug for RawTier<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        formatter.write_char('[')?;

        for i in 0..self.buffer.len() {
            if let Some(elem) = self.get(i) {
                formatter.write_str(format!("{:?}", elem).as_str())?;
            } else {
                formatter.write_str("_")?;
            }

            if i != self.buffer.len() - 1 {
                formatter.write_str(", ")?;
            }
        }

        formatter.write_char(']')?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::tier::*;

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

        assert!(t.push_back(0).is_ok());
        assert!(t.contains_rank(0));
        assert!(t.push_back(1).is_ok());
        assert!(t.contains_rank(0));
        assert!(t.contains_rank(1));
        assert!(t.push_back(2).is_ok());
        assert!(t.contains_rank(0));
        assert!(t.contains_rank(1));
        assert!(t.contains_rank(2));
        assert!(!t.contains_rank(3));
    }

    #[test]
    fn insert_at_rank_shift_head() {
        let mut t = Tier::new(4);

        // [0, 1, 2, n]
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());

        // [1, 3, 2, 0]
        assert!(t.insert(1, 3).is_ok());
        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(*t.get(1).unwrap(), 3);
        assert_eq!(*t.get(2).unwrap(), 2);
        assert_eq!(*t.get(3).unwrap(), 0);
        assert_eq!(t.masked_head(), 3);
    }

    #[test]
    fn insert_at_rank_shift_tail() {
        let mut t = Tier::new(4);

        // [0, 1, 2, n]
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());

        // [0, 1, 3, 2]
        assert!(t.insert(2, 3).is_ok());
        assert_eq!(*t.get(0).unwrap(), 0);
        assert_eq!(*t.get(1).unwrap(), 1);
        assert_eq!(*t.get(2).unwrap(), 3);
        assert_eq!(*t.get(3).unwrap(), 2);
    }

    #[test]
    fn remove_at_rank_1() {
        let mut t = Tier::new(4);

        // [0, 1, 2, 3]
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());
        assert!(t.push_back(3).is_ok());
        assert_eq!(t.masked_head(), 0);
        assert_eq!(t.masked_tail(), 0);

        // [0, 2, 3, _]
        assert!(t.remove(1).is_ok());
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
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());
        assert!(t.push_back(3).is_ok());
        assert_eq!(t.masked_head(), 0);
        assert_eq!(t.masked_tail(), 0);

        // [_, 1, 2, 3]
        assert!(t.remove(0).is_ok());
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
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());

        // [1, 2, n, 0]
        t.shift_to_head(2);
        assert_eq!(*t.get(0).unwrap(), 1);
        assert_eq!(*t.get(1).unwrap(), 2);
        assert_ne!(*t.get(2).unwrap(), 2);
        assert_eq!(*t.get(3).unwrap(), 0);
    }

    #[test]
    fn shift_to_head_data_middle_1() {
        let mut t = Tier::new(4);

        // [n, 1, 2, n]
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());
        assert!(t.pop_front().is_ok());

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
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());
        assert!(t.pop_front().is_ok());

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
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());

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
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(0).is_ok());
        assert!(t.push_back(1).is_ok());
        assert!(t.push_back(2).is_ok());
        assert!(t.pop_front().is_ok());
        assert!(t.pop_front().is_ok());
        assert!(t.push_back(3).is_ok());

        // [n, 3, 1, 2]
        t.shift_to_tail(0);
        assert_ne!(*t.get(0).unwrap(), 3);
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
        assert!(t.push_front(0).is_ok());
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
        assert!(t.push_back(1).is_ok());
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
        assert!(t.push_front(2).is_ok());
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
        assert!(t.push_back(3).is_ok());
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

        assert!(t.push_back(4).is_err());
        assert_eq!(t.masked_head(), 2);
        assert_eq!(t.masked_tail(), 2);

        // [1, 3, n, 0]
        let mut v = t.pop_front();
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 2);
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
        assert!(v.is_ok());
        assert_eq!(v.unwrap(), 3);
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
        assert!(t.push_front(4).is_ok());
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