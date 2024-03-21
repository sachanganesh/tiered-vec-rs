use std::{fmt::Debug, mem::MaybeUninit};

#[repr(align(64))]
#[derive(Clone)]
pub(crate) struct ImplicitTierOffset {
    head: usize,
    tail: usize,
}

impl ImplicitTierOffset {
    #[inline]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline]
    pub const fn is_full(&self, capacity: usize) -> bool {
        self.len() == capacity
    }

    #[inline]
    fn head_forward(&mut self) {
        self.head = self.head.wrapping_add(1).into();
    }

    #[inline]
    fn head_backward(&mut self) {
        self.head = self.head.wrapping_sub(1).into();
    }

    #[inline]
    fn tail_forward(&mut self) {
        self.tail = self.tail.wrapping_add(1).into();
    }

    #[inline]
    fn tail_backward(&mut self) {
        self.tail = self.tail.wrapping_sub(1).into();
    }
}

impl Default for ImplicitTierOffset {
    fn default() -> Self {
        Self { head: 0, tail: 0 }
    }
}

pub struct ImplicitTieredVec<T> {
    offsets: Box<[ImplicitTierOffset]>,
    buffer: Box<[MaybeUninit<T>]>,
}

impl<T> ImplicitTieredVec<T>
where
    T: Clone + Debug + Send + Sync + 'static,
{
    pub fn new(initial_capacity: usize) -> Self {
        let offsets = vec![ImplicitTierOffset::default(); initial_capacity];

        let mut buffer = Vec::with_capacity(initial_capacity);
        unsafe {
            buffer.set_len(initial_capacity);
        }

        Self {
            offsets: offsets.into_boxed_slice(),
            buffer: buffer.into_boxed_slice(),
        }
    }

    pub const fn capacity(&self) -> usize {
        self.buffer.len()
    }

    pub fn len(&self) -> usize {
        let mut l = 0;

        for offset in self.offsets.iter() {
            let offset_len = offset.len();

            if offset_len == 0 {
                break;
            }

            l += offset_len;
        }

        return l;
    }

    pub const fn is_empty(&self) -> bool {
        self.offsets[0].is_empty()
    }

    pub const fn is_full(&self) -> bool {
        self.offsets[self.offsets.len()].is_full(self.offsets.len())
    }

    #[inline]
    const fn mask(&self, val: usize) -> usize {
        val & (self.capacity() - 1)
    }

    pub fn get_by_rank(&self) -> Option<&T> {
        todo!()
    }

    pub fn get_mut_by_rank(&mut self) -> Option<&mut T> {
        todo!()
    }

    fn expand(&mut self) {
        todo!()
    }

    fn contract(&mut self) {
        todo!()
    }

    fn insert(&mut self, rank: usize, elem: T) {
        todo!()
    }

    fn remove(&mut self, rank: usize, elem: T) {
        todo!()
    }
}
