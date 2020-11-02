use super::vector::PackedIntVec;

use super::traits::*;

#[derive(Debug, Default, Clone)]
pub struct PackedDeque {
    vector: PackedIntVec,
    start_ix: usize,
    num_entries: usize,
}

impl PackedDeque {
    const FACTOR: f64 = 1.25;

    pub fn new() -> Self {
        Default::default()
    }

    pub(super) fn reserve(&mut self, capacity: usize) {
        if capacity > self.vector.len() {
            let mut vector = PackedIntVec::new();
            vector.resize(capacity);
            for i in 0..self.num_entries {
                vector.set(i, self.get(i));
            }

            std::mem::swap(&mut vector, &mut self.vector);
            self.start_ix = 0;
        }
    }

    fn grow_as_needed(&mut self) {
        if self.num_entries == self.vector.len() {
            let capacity = Self::FACTOR * self.vector.len() as f64;
            let capacity = 1 + capacity as usize;
            self.reserve(capacity);
        }
    }

    #[inline]
    pub fn push_front(&mut self, value: u64) {
        self.grow_as_needed();

        self.start_ix = if self.start_ix == 0 {
            self.vector.len() - 1
        } else {
            self.start_ix - 1
        };

        self.num_entries += 1;

        self.set(0, value);
    }

    #[inline]
    pub fn push_back(&mut self, value: u64) {
        self.grow_as_needed();

        self.num_entries += 1;

        self.set(self.num_entries - 1, value);
    }

    #[inline]
    pub fn pop_front(&mut self) {
        if self.num_entries > 0 {
            self.start_ix += 1;

            if self.start_ix == self.vector.len() {
                self.start_ix = 0;
            }

            self.num_entries -= 1;
            self.contract();
        }
    }

    #[inline]
    pub fn pop_back(&mut self) {
        if self.num_entries > 0 {
            self.num_entries -= 1;
            self.contract();
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        Iter::new(self)
    }

    #[inline]
    fn contract(&mut self) {
        let capacity = self.vector.len() as f64 / Self::FACTOR.powi(2);
        let capacity = capacity as usize;
        if self.num_entries <= capacity {
            let mut vector = PackedIntVec::new();
            vector.resize(self.num_entries);
            for i in 0..self.num_entries {
                vector.set(i, self.get(i));
            }

            std::mem::swap(&mut vector, &mut self.vector);
            self.start_ix = 0;
        }
    }

    #[inline]
    fn internal_index(&self, ix: usize) -> usize {
        assert!(ix < self.num_entries);
        if let Some(ix) = ix.checked_sub(self.vector.len() - self.start_ix) {
            ix
        } else {
            self.start_ix + ix
        }
        // if ix < self.vector.len() - self.start_ix {
        // } else {
        //     ix - (self.vector.len() - self.start_ix)
        // }
    }
}

impl PackedCollection for PackedDeque {
    #[inline]
    fn len(&self) -> usize {
        self.num_entries
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn set(&mut self, ix: usize, value: u64) {
        self.vector.set(self.internal_index(ix), value);
    }

    #[inline]
    fn get(&self, ix: usize) -> u64 {
        self.vector.get(self.internal_index(ix))
    }

    #[inline]
    fn append(&mut self, value: u64) {
        self.push_back(value)
    }

    #[inline]
    fn pop(&mut self) {
        self.pop_back()
    }

    #[inline]
    fn clear(&mut self) {
        self.vector.clear();
        self.num_entries = 0;
        self.start_ix = 0;
    }
}

pub struct Iter<'a> {
    deque: &'a PackedDeque,
    index: usize,
    len: usize,
}

impl<'a> Iter<'a> {
    fn new(deque: &'a PackedDeque) -> Self {
        Self {
            deque,
            index: 0,
            len: deque.len(),
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<u64> {
        if self.index < self.len {
            let item = self.deque.get(self.index);
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}

impl<'a> std::iter::FusedIterator for Iter<'a> {}

impl std::iter::FromIterator<u64> for PackedDeque {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let mut deque = PackedDeque::new();
        iter.into_iter().for_each(|v| deque.push_back(v));
        deque
    }
}
