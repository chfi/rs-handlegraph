use succinct::{IntVec, IntVecMut, IntVector};

#[derive(Debug, Clone)]
pub struct PackedIntVec {
    vector: IntVector<u64>,
    num_entries: usize,
    width: usize,
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

    #[inline]
    pub fn len(&self) -> usize {
        self.num_entries
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn clear(&mut self) {
        self.width = 1;
        self.vector = IntVector::new(self.width);
        self.num_entries = 0;
    }

    #[inline]
    pub fn resize(&mut self, size: usize) {
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

    #[inline]
    pub fn reserve(&mut self, size: usize) {
        if size > self.vector.len() as usize {
            self.vector.resize(size as u64, 0);
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u64) {
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
    pub fn get(&self, index: usize) -> u64 {
        assert!(index < self.num_entries);
        self.vector.get(index as u64)
    }

    #[inline]
    pub fn append(&mut self, value: u64) {
        self.resize(self.num_entries + 1);
        self.set(self.num_entries - 1, value);
    }

    #[inline]
    pub fn pop(&mut self) {
        self.resize(self.num_entries - 1);
    }
}

impl PartialEq for PackedIntVec {
    #[inline]
    fn eq(&self, other: &PackedIntVec) -> bool {
        self.vector == other.vector
    }
}

use quickcheck::{Arbitrary, Gen};

impl Arbitrary for PackedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> PackedIntVec {
        let mut intvec = PackedIntVec::new();
        let u64_vec: Vec<u64> = Vec::arbitrary(g);

        for v in u64_vec {
            intvec.append(v);
        }
        intvec
    }
}

#[cfg(test)]
mod tests {

    use quickcheck::quickcheck;

    use super::*;

    #[test]
    fn test_append() {
        let mut intvec = PackedIntVec::new();
        assert_eq!(intvec.len(), 0);
        assert_eq!(intvec.width(), 1);

        intvec.append(1);
        assert_eq!(intvec.len(), 1);
        assert_eq!(intvec.width(), 1);

        intvec.append(2);
        assert_eq!(intvec.len(), 2);
        assert_eq!(intvec.width(), 2);

        intvec.append(10);
        assert_eq!(intvec.len(), 3);
        assert_eq!(intvec.width(), 4);

        intvec.append(120);
        assert_eq!(intvec.len(), 4);
        assert_eq!(intvec.width(), 7);

        intvec.append(3);
        assert_eq!(intvec.len(), 5);
        assert_eq!(intvec.width(), 7);
    }

    quickcheck! {
        fn prop_append(intvec: PackedIntVec, value: u64) -> bool {
            let mut intvec = intvec;

            let filled_before = intvec.len();
            let width_before = intvec.width();

            intvec.append(value);

            let filled_correct = intvec.len() == filled_before + 1;

            let last_val = intvec.get(intvec.len() - 1);

            let width_after = intvec.width();

            filled_correct && last_val == value && width_after >= width_before
        }
    }

    quickcheck! {
        fn prop_pop(intvec: PackedIntVec) -> bool {
            let mut intvec = intvec;

            let filled_before = intvec.len();
            let width_before = intvec.width();

            intvec.pop();

            let filled_after = intvec.len();
            let width_after = intvec.width();

            filled_after == filled_before - 1 &&
                width_before == width_after
        }
    }

    quickcheck! {
        fn prop_get(vector: Vec<u64>) -> bool {
            let mut intvec = PackedIntVec::new();
            for &x in vector.iter() {
                intvec.append(x);
            }

            for ix in 0..vector.len() {
                let a = vector[ix];
                let b = intvec.get(ix);
                if a != b {
                    return false;
                }
            }

            true
        }
    }
}
