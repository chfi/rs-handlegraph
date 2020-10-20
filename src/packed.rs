use succinct::{IntVec, IntVecMut, IntVector};

pub struct PackedIntVec {
    vector: IntVector<u64>,
    filled: usize,
    width: usize,
}

impl PackedIntVec {
    pub fn new() -> Self {
        let width = 1;
        let vector = IntVector::new(width);
        let filled = 0;
        PackedIntVec {
            vector,
            filled,
            width,
        }
    }

    pub fn set(&mut self, index: usize, value: u64) {
        assert!(index < self.filled);

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

            for ix in 0..(self.filled as u64) {
                new_vec.set(ix, self.vector.get(ix));
            }
            std::mem::swap(&mut self.vector, &mut new_vec);
        }

        self.vector.set(index as u64, value);
    }
}
