use succinct::{IntVec, IntVecMut, IntVector};

pub trait PackedCollection {
    fn len(&self) -> usize;

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set(&mut self, index: usize, value: u64);

    fn get(&self, index: usize) -> u64;

    fn append(&mut self, value: u64);

    fn pop(&mut self);

    fn clear(&mut self);


}
