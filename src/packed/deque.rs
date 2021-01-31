use super::traits::*;
use super::vector::PackedIntVec;

#[derive(Debug, Default, Clone)]
pub struct PackedDeque {
    vector: PackedIntVec,
    start_ix: usize,
    num_entries: usize,
}

crate::impl_space_usage!(PackedDeque, [vector]);

impl PackedDeque {
    const FACTOR: f64 = 1.25;

    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_width(width: usize) -> Self {
        let vector = PackedIntVec::new_with_width(width);
        Self {
            vector,
            ..Default::default()
        }
    }

    pub fn with_width_and_capacity(width: usize, capacity: usize) -> Self {
        let vector = PackedIntVec::with_width_and_capacity(width, capacity);
        Self {
            vector,
            ..Default::default()
        }
    }

    #[inline]
    pub fn reserve_with_width(&mut self, width: usize, capacity: usize) {
        if capacity > self.vector.len() {
            let mut vector = PackedIntVec::new_with_width(width);
            vector.resize(capacity);
            for i in 0..self.num_entries {
                vector.set(i, self.get(i));
            }

            std::mem::swap(&mut vector, &mut self.vector);
            self.start_ix = 0;
        }
    }

    #[inline]
    pub fn reserve(&mut self, capacity: usize) {
        if capacity > self.vector.len() {
            let width = self.vector.width();
            let mut vector = PackedIntVec::new_with_width(width);
            vector.resize(capacity);
            for i in 0..self.num_entries {
                vector.set(i, self.get(i));
            }

            std::mem::swap(&mut vector, &mut self.vector);
            self.start_ix = 0;
        }
    }

    #[inline]
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
        let diff = self.vector.len() - self.start_ix;
        if ix >= diff {
            ix - diff
        } else {
            self.start_ix + ix
        }
    }

    pub fn save_diagnostics<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> std::io::Result<()> {
        let mut min = std::u64::MAX;
        let mut max = 0u64;
        let mut non_zero_elems = 0;

        for v in self.vector.iter() {
            if v != 0 {
                min = min.min(v);
                non_zero_elems += 1;
            }

            max = max.max(v);
        }

        writeln!(&mut w, "# non-zero elements {}", non_zero_elems)?;
        writeln!(&mut w, "# width {}", self.vector.width())?;
        writeln!(&mut w, "# min value {}", min)?;
        writeln!(&mut w, "# max value {}", max)?;

        Ok(())
    }

    pub fn print_diagnostics(&self) {
        let mut min = std::u64::MAX;
        let mut max = 0u64;
        let mut non_zero_elems = 0;

        for v in self.vector.iter() {
            if v != 0 {
                min = min.min(v);
                non_zero_elems += 1;
            }

            max = max.max(v);
        }

        println!(
            "Elements {:6}\tWidth {:2}\tMin {:6}\tMax {:6}",
            non_zero_elems,
            self.vector.width(),
            min,
            max
        );
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
    deque_vec: &'a PackedIntVec,
    start_ix: usize,
    cur_ix: usize,
    len: usize,
    diff: usize,
    ix_gte_diff: bool,
    finished: bool,
}

impl<'a> Iter<'a> {
    fn new(deque: &'a PackedDeque) -> Self {
        let diff = deque.vector.len() - deque.start_ix;

        Self {
            deque_vec: &deque.vector,
            cur_ix: 0,
            start_ix: deque.start_ix,
            len: deque.len(),
            diff,
            ix_gte_diff: false,
            finished: false,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = u64;

    #[inline]
    fn next(&mut self) -> Option<u64> {
        if self.finished {
            return None;
        }

        let cur_ix = self.cur_ix;
        self.cur_ix += 1;

        let ix = if !self.ix_gte_diff {
            if self.cur_ix == self.diff {
                self.ix_gte_diff = true;
            }
            cur_ix + self.start_ix
        } else {
            cur_ix - self.diff
        };

        self.finished |= self.cur_ix == self.len;

        Some(self.deque_vec.get(ix))
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

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck::{quickcheck, Arbitrary, Gen};

    impl Arbitrary for PackedDeque {
        fn arbitrary<G: Gen>(g: &mut G) -> PackedDeque {
            let front: Vec<u64> = Vec::arbitrary(g);
            let back: Vec<u64> = Vec::arbitrary(g);
            let front_first = bool::arbitrary(g);

            let mut deque = PackedDeque::new();

            if front_first {
                front.into_iter().for_each(|v| deque.push_front(v));
                back.into_iter().for_each(|v| deque.push_back(v));
            } else {
                back.into_iter().for_each(|v| deque.push_back(v));
                front.into_iter().for_each(|v| deque.push_front(v));
            }
            deque
        }
    }

    quickcheck! {
        fn prop_deque_push_front(deque: PackedDeque, val: u64) -> bool {
            let mut deque = deque;
            let len = deque.len();

            deque.push_front(val);

            deque.len() == len + 1 &&
            deque.get(0) == val
        }
    }

    quickcheck! {
        fn prop_deque_push_back(deque: PackedDeque, val: u64) -> bool {
            let mut deque = deque;
            let len = deque.len();

            deque.push_back(val);

            deque.len() == len + 1 &&
                deque.get(deque.len() - 1) == val
        }
    }

    quickcheck! {
        fn prop_deque_pop_back(deque: PackedDeque) -> bool {
            let mut deque = deque;
            let len = deque.len();

            if len <= 1 {
                deque.pop_back();
                deque.len() == 0
            } else {
                let second_last = deque.get(deque.len() - 2);
                deque.pop_back();
                deque.len() == len - 1 &&
                    deque.get(deque.len() - 1) == second_last
            }
        }
    }

    quickcheck! {
        fn prop_deque_pop_front(deque: PackedDeque) -> bool {
            let mut deque = deque;
            let len = deque.len();

            if len <= 1 {
                deque.pop_front();
                deque.len() == 0
            } else {
                let second = deque.get(1);
                deque.pop_front();

                deque.len() == len - 1 &&
                    deque.get(0) == second
            }
        }
    }
}
