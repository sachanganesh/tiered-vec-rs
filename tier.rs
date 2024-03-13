use crossbeam_utils::CachePadded;
use std::{
    fmt::Debug,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum TierError<T>
where
    T: Debug + Send + Sync,
{
    #[error("tier is full and cannot be inserted into")]
    TierInsertionError(T),

    #[error("tier is empty and no element can be removed")]
    TierEmptyError,
    //
    // #[error("the provided index is out of bounds")]
    // TierIndexOutOfBoundsError(usize),
    //
    // #[error("tier is full and at least some elements cannot be inserted")]
    // TierMultipleInsertionError(Vec<T>),
}

pub struct Tier<T, const N: usize> {
    inner: CachePadded<RawTier<T, N>>,
}

impl<T, const N: usize> Tier<T, N>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: CachePadded::new(RawTier::new()),
        }
    }
}

impl<T, const N: usize> Deref for Tier<T, N> {
    type Target = RawTier<T, N>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T, const N: usize> DerefMut for Tier<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.deref_mut()
    }
}

// #[repr(align(64))]
pub struct RawTier<T, const N: usize> {
    pub(crate) arr: [MaybeUninit<T>; N],
    pub(crate) head: usize,
    pub(crate) tail: usize,
}

impl<T, const N: usize> RawTier<T, N>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub(crate) fn new() -> Self {
        // let mut arr = Vec::with_capacity(max_size);
        // arr.resize_with(max_size, || None::<T>);
        // let arr = Box::from_raw(Box::into_raw(Vec::with_capacity(max_size).into_boxed_slice()) as *mut [Option<T>; max_size])

        // assert!(capacity.is_power_of_two());
        // let mut vec = Vec::with_capacity(capacity);
        // unsafe {
        //     vec.set_len(capacity);
        // }
        // let arr = vec.into_boxed_slice();

        assert!(N.is_power_of_two());
        let arr: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        // let arr = vec.into_boxed_slice();

        Self {
            arr,
            head: 0,
            tail: 0,
        }
    }

    pub const fn capacity(&self) -> usize {
        self.arr.len()
    }

    pub const fn len(&self) -> usize {
        self.tail - self.head
    }

    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    pub const fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    fn valid_index(&self, masked_idx: usize) -> bool {
        // passed idx should be already masked
        let masked_head = self.mask(self.head);
        let masked_tail = self.mask(self.tail);

        if masked_head <= masked_tail {
            masked_tail == 0 || (masked_idx >= masked_head && masked_idx < masked_tail)
        } else {
            masked_idx < masked_tail || masked_idx >= masked_head
        }
    }

    const fn has_previously_been_written_to(&self, idx: usize) -> bool {
        let rel_idx = self.mask(self.head.wrapping_add(idx));
        return rel_idx < self.tail;
    }

    const fn mask(&self, val: usize) -> usize {
        val & (self.arr.len() - 1)
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        let masked_idx = self.mask(idx);
        if !self.valid_index(masked_idx) {
            return None;
        }

        let elem = &self.arr[masked_idx];
        Some(unsafe { elem.assume_init_ref() })
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        let masked_idx = self.mask(idx);
        if !self.valid_index(masked_idx) {
            return None;
        }

        let elem = &mut self.arr[masked_idx];
        Some(unsafe { elem.assume_init_mut() })
    }

    pub fn push_back(&mut self, elem: T) -> Result<usize, TierError<T>> {
        if !self.is_full() {
            let idx = self.mask(self.tail);
            self.tail = self.tail.wrapping_add(1);

            // if self.has_previously_been_written_to(idx) {
            //     unsafe {
            //         self.arr[idx].assume_init_drop();
            //     }
            // }

            self.arr[idx].write(elem);
            Ok(idx)
        } else {
            Err(TierError::TierInsertionError(elem).into())
        }
    }

    pub fn pop_front(&mut self) -> Result<T, TierError<T>> {
        if !self.is_empty() {
            let idx = self.mask(self.head);
            self.head = self.head.wrapping_add(1);

            let slot = &mut self.arr[idx];
            let elem = unsafe { std::mem::replace(slot, MaybeUninit::uninit()).assume_init() };

            Ok(elem)
        } else {
            Err(TierError::TierEmptyError)
        }
    }

    // pub(crate) fn replace(&mut self, elem: T, idx: usize) -> Option<T> {
    //     let replaced = std::mem::replace(&mut self.arr[self.tail_idx], Some(elem));
    //     self.tail_idx += 1;

    //     if replaced.is_some() {
    //         self.tombstones -= 1;
    //     }

    //     return replaced;
    // }

    // pub fn insert_at(&mut self, elem: T, idx: usize) -> Result<(), TierError<T>> {
    //     if !self.is_full() && idx < self.arr.len() {
    //         self.arr[0] = Some(elem);
    //         Ok(())
    //     } else {
    //         Err(TierError::TierInsertionError(elem).into())
    //     }
    // }

    // pub fn insert_vec_at<U>(&mut self, elems: &Vec<T>, idx: usize) -> Result<(), TierError<T>> where T: Clone {
    //     if !self.is_full() && idx < self.arr.len() && (self.arr.len() - self.capacity()) >= 1 {
    //         for elem in elems.clone_from_slice() {
    //             self.arr[idx] = Some(elem);
    //         }
    //         Ok(())
    //     } else {
    //         Err(TierError::TierInsertionError(elem).into())
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use crate::cache_conscious::tiered_vec::tier::*;

    #[test]
    #[should_panic]
    fn error_on_wrong_tier_size() {
        let _t: Tier<usize, 5> = Tier::new();
    }

    #[test]
    fn no_error_on_correct_tier_size() {
        let _t: Tier<usize, 4> = Tier::new();
    }

    #[test]
    fn push_and_pop() {
        let mut t: Tier<usize, 4> = Tier::new();
        assert!(t.is_empty());
        assert!(!t.is_full());
        assert_eq!(t.len(), 0);
        assert_eq!(t.capacity(), 4);

        assert!(t.push_back(0).is_ok());
        assert_eq!(t.inner.head, 0);
        assert_eq!(t.inner.tail, 1);

        assert!(t.push_back(1).is_ok());
        assert_eq!(t.inner.head, 0);
        assert_eq!(t.inner.tail, 2);

        assert!(t.push_back(2).is_ok());
        assert_eq!(t.inner.head, 0);
        assert_eq!(t.inner.tail, 3);

        assert!(t.push_back(3).is_ok());
        assert_eq!(t.inner.head, 0);
        assert_eq!(t.inner.tail, 4);

        assert!(!t.is_empty());
        assert!(t.is_full());
        assert_eq!(t.len(), 4);

        assert!(!t.push_back(4).is_ok());
        assert_eq!(t.inner.head, 0);
        assert_eq!(t.inner.tail, 4);

        assert_eq!(*t.get(0).unwrap(), 0usize);
        assert_eq!(*t.get_mut(1).unwrap(), 1usize);
        assert_eq!(*t.get(2).unwrap(), 2usize);
        assert_eq!(*t.get_mut(3).unwrap(), 3usize);

        assert!(t.pop_front().is_ok());
        assert!(!t.is_empty());
        assert!(!t.is_full());
        assert!(t.get(0).is_none());
        assert_eq!(t.len(), 3);

        assert!(t.pop_front().is_ok());
        assert!(t.get(1).is_none());

        assert!(t.push_back(4).is_ok());
        assert!(t.get(4).is_some());
        assert_eq!(*t.get(4).unwrap(), 4);
        assert_eq!(t.get(0).unwrap(), t.get(4).unwrap());
    }
}
