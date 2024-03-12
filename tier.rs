use std::fmt::Debug;
use thiserror::Error;

pub(crate) struct Tier<T> {
    pub(crate) tombstones: usize,
    pub(crate) tail_idx: usize,
    pub(crate) arr: Box<[Option<T>]>,
}

#[derive(Debug, Error)]
pub(crate) enum TierError<T>
where
    T: Debug + Send + Sync,
{
    #[error("tier is full and cannot be inserted into")]
    TierInsertionError(T),

    #[error("tier is full and at least some elements cannot be inserted")]
    TierMultipleInsertionError(Vec<T>),
}

impl<T> Tier<T>
where
    T: Debug + Send + Sync + 'static,
{
    pub fn new(max_size: usize) -> Self {
        // let mut arr = Vec::with_capacity(max_size);
        // arr.resize_with(max_size, || None::<T>);
        // let arr = Box::from_raw(Box::into_raw(Vec::with_capacity(max_size).into_boxed_slice()) as *mut [Option<T>; max_size])

        let arr = Vec::with_capacity(max_size).into_boxed_slice();

        Self {
            tombstones: max_size,
            tail_idx: 0,
            arr,
        }
    }

    const fn capacity(&self) -> usize {
        self.tombstones
    }

    const fn len(&self) -> usize {
        self.arr.len() - self.capacity()
    }

    const fn is_full(&self) -> bool {
        self.capacity() == 0
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        self.arr.get(idx)?.as_ref()
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.arr.get_mut(idx)?.as_mut()
    }

    pub(crate) fn replace(&mut self, elem: T, idx: usize) -> Option<T> {
        let replaced = std::mem::replace(&mut self.arr[self.tail_idx], Some(elem));
        self.tail_idx += 1;

        if replaced.is_some() {
            self.tombstones -= 1;
        }

        return replaced;
    }

    pub fn push(&mut self, elem: T) -> Result<usize, TierError<T>> {
        if !self.is_full() {
            let res = Ok(self.tail_idx);
            self.tail_idx += 1;

            res
        } else {
            Err(TierError::TierInsertionError(elem).into())
        }
    }

    pub fn insert_at(&mut self, elem: T, idx: usize) -> Result<(), TierError<T>> {
        if !self.is_full() && idx < self.arr.len() {
            self.arr[0] = Some(elem);
            Ok(())
        } else {
            Err(TierError::TierInsertionError(elem).into())
        }
    }

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
