use succinct::{IntVec, IntVecMut, IntVector};

pub struct PackedIntVec {
    vector: IntVector<u64>,
    filled_elements: usize,
    width: usize,
}

impl PackedIntVec {
    const FACTOR: f64 = 1.25;

    pub fn new() -> Self {
        let width = 1;
        let vector = IntVector::new(width);
        let filled_elements = 0;
        PackedIntVec {
            vector,
            filled_elements,
            width,
        }
    }

    pub fn len(&self) -> usize {
        self.filled_elements
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&mut self) {
        self.width = 1;
        self.vector = IntVector::new(self.width);
        self.filled_elements = 0;
    }

    pub fn resize(&mut self, size: usize) {
        if size < self.filled_elements {
            let capacity = self.vector.len() as f64 / (Self::FACTOR.powi(2));
            let capacity = capacity as usize;
            if size < capacity {
                let mut new_vec: IntVector<u64> =
                    IntVector::with_capacity(self.width, self.vector.len());
                for ix in 0..(self.filled_elements as u64) {
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

        self.filled_elements = size;
    }

    pub fn reserve(&mut self, size: usize) {
        if size > self.vector.len() as usize {
            self.vector.resize(size as u64, 0);
        }
    }

    pub fn set(&mut self, index: usize, value: u64) {
        assert!(index < self.filled_elements);

        let mut new_width = self.width;
        let max = std::u64::MAX;
        let mut mask = max << new_width;

        while (mask & value) != 0 {
            new_width += 1;
            mask = max << new_width;
        }

        if new_width > self.width {
            self.width = new_width;

            let mut new_vec: IntVector<u64> =
                IntVector::with_capacity(new_width, self.vector.len());

            for ix in 0..(self.filled_elements as u64) {
                new_vec.set(ix, self.vector.get(ix));
            }
            std::mem::swap(&mut self.vector, &mut new_vec);
        }

        self.vector.set(index as u64, value);
    }

    pub fn get(&self, index: usize) -> u64 {
        assert!(index < self.filled_elements);
        self.vector.get(index as u64)
    }

    pub fn append(&mut self, value: u64) {
        self.resize(self.filled_elements);
        self.set(self.filled_elements - 1, value);
    }

    pub fn pop(&mut self) {
        self.resize(self.filled_elements - 1);
    }
}

impl PartialEq for PackedIntVec {
    fn eq(&self, other: &PackedIntVec) -> bool {
        self.vector == other.vector
    }
}
