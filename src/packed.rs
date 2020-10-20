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

        let mut width = self.width;
        let max = std::u64::MAX;
        let mut mask = max << width;

        while (mask & value) != 0 {
            width += 1;
            mask = max << width;
        }

        if width > self.width {
            let mut new_vec: IntVector<u64> =
                IntVector::with_capacity(width, self.vector.len());

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

    // pub fn append(&mut self, value: u64) {
    // }
}
