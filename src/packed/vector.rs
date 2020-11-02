use succinct::{IntVec, IntVecMut, IntVector};

use super::traits::*;

#[derive(Debug, Clone)]
pub struct PackedIntVec {
    vector: IntVector<u64>,
    num_entries: usize,
    width: usize,
}

impl PartialEq for PackedIntVec {
    #[inline]
    fn eq(&self, other: &PackedIntVec) -> bool {
        self.vector == other.vector
    }
}

impl Default for PackedIntVec {
    fn default() -> PackedIntVec {
        let width = 1;
        let vector = IntVector::new(width);
        let num_entries = 0;
        PackedIntVec {
            vector,
            num_entries,
            width,
        }
    }
}

impl PackedIntVec {
    const FACTOR: f64 = 1.25;

    pub fn new() -> Self {
        Default::default()
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.width
    }

    pub(super) fn resize(&mut self, size: usize) {
        if size < self.num_entries {
            let capacity = self.vector.len() as f64 / (Self::FACTOR.powi(2));
            let capacity = capacity as usize;
            if size < capacity {
                let mut new_vec: IntVector<u64> =
                    IntVector::with_capacity(self.width, self.vector.len());
                for ix in 0..(self.num_entries as u64) {
                    new_vec.set(ix, self.vector.get(ix));
                }
                std::mem::swap(&mut self.vector, &mut new_vec);
            }
        } else if size > self.vector.len() as usize {
            let fac_size = self.vector.len() as f64 * Self::FACTOR;
            let fac_size = fac_size as usize + 1;
            let new_cap = size.max(fac_size);
            self.reserve(new_cap);
        }

        self.num_entries = size;
    }

    pub(super) fn reserve(&mut self, size: usize) {
        if size > self.vector.len() as usize {
            self.vector.resize(size as u64, 0);
        }
    }

    pub fn iter(&self) -> Iter<'_> {
        let iter = self.vector.iter();
        Iter::new(iter, self.num_entries)
    }

    pub fn iter_slice(&self, offset: usize, length: usize) -> Iter<'_> {
        let iter = self.vector.iter();
        Iter::offset_new(iter, offset, length)
    }
}

impl PackedCollection for PackedIntVec {
    #[inline]
    fn len(&self) -> usize {
        self.num_entries
    }
    #[inline]
    fn clear(&mut self) {
        self.width = 1;
        self.vector = IntVector::new(self.width);
        self.num_entries = 0;
    }

    #[inline]
    fn set(&mut self, index: usize, value: u64) {
        assert!(index < self.num_entries);

        let new_width = 64 - value.leading_zeros() as usize;

        if new_width > self.width {
            self.width = new_width;

            let mut new_vec: IntVector<u64> =
                IntVector::with_capacity(new_width, self.vector.len());

            for ix in 0..(self.num_entries as u64) {
                new_vec.push(self.vector.get(ix));
            }
            std::mem::swap(&mut self.vector, &mut new_vec);
        }

        self.vector.set(index as u64, value);
    }

    #[inline]
    fn get(&self, index: usize) -> u64 {
        assert!(index < self.num_entries);
        self.vector.get(index as u64)
    }

    #[inline]
    fn append(&mut self, value: u64) {
        self.resize(self.num_entries + 1);
        self.set(self.num_entries - 1, value);
    }

    #[inline]
    fn pop(&mut self) {
        if let Some(new_size) = self.num_entries.checked_sub(1) {
            self.resize(new_size);
        }
    }
}

pub struct Iter<'a> {
    iter: succinct::int_vec::Iter<'a, u64>,
    left_ix: usize,
    right_ix: usize,
}

impl<'a> Iter<'a> {
    fn new(iter: succinct::int_vec::Iter<'a, u64>, num_entries: usize) -> Self {
        let left_ix = 0;
        let right_ix = num_entries;
        Self {
            iter,
            left_ix,
            right_ix,
        }
    }

    fn offset_new(
        mut iter: succinct::int_vec::Iter<'a, u64>,
        offset: usize,
        length: usize,
    ) -> Self {
        let drop_right = iter.len() - (offset + length);
        for _ in 0..drop_right {
            iter.next_back();
        }
        let left_ix = offset;
        let right_ix = offset + length;
        for _ in 0..offset {
            iter.next();
        }
        Self {
            iter,
            left_ix,
            right_ix,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        if self.left_ix < self.right_ix {
            let item = self.iter.next();
            self.left_ix += 1;
            item
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = if self.left_ix < self.right_ix {
            self.right_ix - self.left_ix
        } else {
            0
        };
        let upper = Some(lower);
        (lower, upper)
    }

    fn count(self) -> usize {
        if self.left_ix < self.right_ix {
            self.right_ix - self.left_ix
        } else {
            0
        }
    }

    fn last(mut self) -> Option<u64> {
        if self.left_ix < self.right_ix {
            self.iter.nth(self.right_ix - self.left_ix)
        } else {
            None
        }
    }

    fn nth(&mut self, n: usize) -> Option<u64> {
        if self.left_ix + n < self.right_ix {
            self.iter.nth(n)
        } else {
            None
        }
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<u64> {
        if self.right_ix > self.left_ix {
            let item = self.iter.next_back();
            self.right_ix += 1;
            item
        } else {
            None
        }
    }
}

impl std::iter::FromIterator<u64> for PackedIntVec {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let mut intvec = PackedIntVec::new();
        iter.into_iter().for_each(|v| intvec.append(v));
        intvec
    }
}

// impl From<Vec<u64>> for PackedIntVec {
//     fn from(vector: Vec<u64>) -> Self {
//         let mut intvec = PackedIntVec::new();
//         intvec.resize(vector.len());
//         for (ix, v) in vector.into_iter().enumerate() {
//             intvec.set(ix, v);
//         }
//         intvec
//     }
// }
