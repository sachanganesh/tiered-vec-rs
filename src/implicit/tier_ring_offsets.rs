#[derive(Clone, Debug)]
pub(crate) struct ImplicitTierRingOffsets {
    head: usize,
    tail: usize,
}

impl ImplicitTierRingOffsets {
    pub fn new(head: usize, tail: usize) -> Self {
        Self { head, tail }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    #[inline(always)]
    pub const fn is_full(&self, capacity: usize) -> bool {
        self.len() == capacity
    }

    #[inline(always)]
    pub const fn head(&self) -> usize {
        self.head
    }

    pub fn set_head(&mut self, new_head: usize) {
        self.head = new_head;
    }

    #[inline(always)]
    pub const fn tail(&self) -> usize {
        self.tail
    }

    pub fn set_tail(&mut self, new_tail: usize) {
        self.tail = new_tail;
    }

    #[inline(always)]
    pub fn head_forward(&mut self) {
        self.head = self.head.wrapping_add(1).into();
    }

    #[inline(always)]
    pub fn head_backward(&mut self) {
        self.head = self.head.wrapping_sub(1).into();
    }

    #[inline(always)]
    pub fn tail_forward(&mut self) {
        self.tail = self.tail.wrapping_add(1).into();
    }

    #[inline(always)]
    pub fn tail_backward(&mut self) {
        self.tail = self.tail.wrapping_sub(1).into();
    }

    #[inline(always)]
    const fn mask(&self, val: usize, capacity: usize) -> usize {
        val & (capacity - 1)
    }

    #[inline(always)]
    pub const fn masked_head(&self, capacity: usize) -> usize {
        self.mask(self.head, capacity)
    }

    #[inline(always)]
    pub const fn masked_tail(&self, capacity: usize) -> usize {
        self.mask(self.tail, capacity)
    }

    #[inline(always)]
    pub const fn masked_rank(&self, rank: usize, capacity: usize) -> usize {
        self.mask(self.head.wrapping_add(rank), capacity)
    }
}

impl Default for ImplicitTierRingOffsets {
    fn default() -> Self {
        Self { head: 0, tail: 0 }
    }
}
