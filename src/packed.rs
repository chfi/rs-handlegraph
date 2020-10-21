use succinct::{IntVec, IntVecMut, IntVector};

#[derive(Debug, Clone)]
pub struct PackedIntVec {
    vector: IntVector<u64>,
    num_entries: usize,
    width: usize,
}

pub struct PackedIntVecIter<'a> {
    iter: succinct::int_vec::Iter<'a, u64>,
    num_entries: usize,
    index: usize,
}

impl<'a> PackedIntVecIter<'a> {
    fn new(iter: succinct::int_vec::Iter<'a, u64>, num_entries: usize) -> Self {
        Self {
            iter,
            num_entries,
            index: 0,
        }
    }
}

impl<'a> Iterator for PackedIntVecIter<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        if self.index < self.num_entries {
            let item = self.iter.next();
            self.index += 1;
            item
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = if self.index < self.num_entries {
            self.num_entries - self.index
        } else {
            0
        };
        let upper = Some(lower);
        (lower, upper)
    }

    fn count(self) -> usize {
        if self.index < self.num_entries {
            self.num_entries - self.index
        } else {
            0
        }
    }

    fn last(mut self) -> Option<u64> {
        if self.index < self.num_entries {
            self.iter.nth(self.num_entries - self.index)
        } else {
            None
        }
    }

    fn nth(&mut self, n: usize) -> Option<u64> {
        if self.index + n < self.num_entries {
            self.iter.nth(n)
        } else {
            None
        }
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
        if self.num_entries > 0 {
            self.resize(self.num_entries - 1);
        }
    }

    pub fn iter(&self) -> PackedIntVecIter<'_> {
        let iter = self.vector.iter();
        PackedIntVecIter::new(iter, self.num_entries)
    }
}

impl PartialEq for PackedIntVec {
    #[inline]
    fn eq(&self, other: &PackedIntVec) -> bool {
        self.vector == other.vector
    }
}

#[derive(Debug, Clone)]
pub struct PagedIntVec {
    page_size: usize,
    num_entries: usize,
    anchors: PackedIntVec,
    pages: Vec<PackedIntVec>,
}

impl Default for PagedIntVec {
    fn default() -> Self {
        Self::new(64)
    }
}

impl PagedIntVec {
    pub fn new(page_size: usize) -> Self {
        let num_entries = 0;
        let pages = Vec::new();
        let anchors = Default::default();
        PagedIntVec {
            page_size,
            num_entries,
            anchors,
            pages,
        }
    }

    #[inline]
    pub fn page_width(&self) -> usize {
        self.page_size
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
        self.pages.clear();
        self.anchors.clear();
        self.num_entries = 0;
    }

    #[inline]
    pub fn resize(&mut self, new_size: usize) {
        if new_size < self.num_entries {
            let num_pages = if new_size == 0 {
                0
            } else {
                (new_size - 1) / self.page_size + 1
            };

            self.anchors.resize(num_pages);
            self.pages.resize_with(num_pages, Default::default);
        } else if new_size > self.num_entries {
            self.reserve(new_size);
        }

        self.num_entries = new_size;
    }

    #[inline]
    pub fn reserve(&mut self, new_size: usize) {
        if new_size > self.pages.len() * self.page_size {
            let num_pages = (new_size - 1) / self.page_size + 1;

            self.anchors.reserve(num_pages);
            self.pages.reserve(num_pages - self.pages.len());

            self.anchors.resize(num_pages);
            while num_pages > self.pages.len() {
                let mut new_page = PackedIntVec::new();
                new_page.resize(self.page_size);
                self.pages.push(new_page);
            }
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u64) {
        assert!(index < self.num_entries);

        let page_ix = index / self.page_size;
        let mut anchor = self.anchors.get(page_ix);

        if anchor == 0 {
            self.anchors.set(page_ix, value);
            anchor = value;
        }

        self.pages[page_ix]
            .set(index % self.page_size, Self::to_diff(value, anchor));
    }

    #[inline]
    pub fn get(&self, index: usize) -> u64 {
        assert!(index < self.num_entries);
        let page_ix = index / self.page_size;
        Self::from_diff(
            self.pages[page_ix].get(index % self.page_size),
            self.anchors.get(page_ix),
        )
    }

    #[inline]
    pub fn append(&mut self, value: u64) {
        if self.num_entries == self.pages.len() * self.page_size {
            let mut new_page = PackedIntVec::new();
            new_page.resize(self.page_size);
            self.anchors.append(0);
            self.pages.push(new_page);
        }

        self.num_entries += 1;
        self.set(self.num_entries - 1, value);
    }

    #[inline]
    pub fn pop(&mut self) {
        if self.num_entries > 0 {
            self.num_entries -= 1;

            while self.num_entries + self.page_size
                <= self.pages.len() * self.page_size
            {
                self.pages.pop();
                self.pages.shrink_to_fit();
                self.anchors.pop();
            }
        }
    }

    #[inline]
    const fn to_diff(value: u64, anchor: u64) -> u64 {
        if value == 0 {
            0
        } else if value >= anchor {
            let raw_diff = value - anchor;
            raw_diff + raw_diff / 4 + 1
        } else {
            5 * (anchor - value)
        }
    }

    #[inline]
    const fn from_diff(diff: u64, anchor: u64) -> u64 {
        if diff == 0 {
            0
        } else if diff % 5 == 0 {
            anchor - diff / 5
        } else {
            anchor + diff - diff / 5 - 1
        }
    }
}

#[derive(Debug, Clone)]
pub struct RobustPagedIntVec {
    first_page: PackedIntVec,
    other_pages: PagedIntVec,
}

impl RobustPagedIntVec {
    pub fn new(page_size: usize) -> Self {
        let first_page = PackedIntVec::new();
        let other_pages = PagedIntVec::new(page_size);
        Self {
            first_page,
            other_pages,
        }
    }

    #[inline]
    pub fn page_width(&self) -> usize {
        self.other_pages.page_width()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.first_page.len() + self.other_pages.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.first_page.is_empty() && self.other_pages.is_empty()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.first_page.clear();
        self.other_pages.clear();
    }

    #[inline]
    pub fn resize(&mut self, new_size: usize) {
        if new_size > self.page_width() {
            self.first_page.resize(self.page_width());
            self.other_pages.resize(new_size - self.page_width());
        } else {
            self.first_page.resize(new_size);
            self.other_pages.clear();
        }
    }

    #[inline]
    pub fn reserve(&mut self, capacity: usize) {
        if capacity > self.page_width() {
            self.first_page.reserve(self.page_width());
            self.other_pages.reserve(capacity - self.page_width());
        } else {
            self.first_page.reserve(capacity);
        }
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u64) {
        if index < self.page_width() {
            self.first_page.set(index, value);
        } else {
            self.other_pages.set(index - self.page_width(), value);
        }
    }

    #[inline]
    pub fn get(&mut self, index: usize) -> u64 {
        if index < self.page_width() {
            self.first_page.get(index)
        } else {
            self.other_pages.get(index - self.page_width())
        }
    }

    #[inline]
    pub fn append(&mut self, value: u64) {
        if self.first_page.len() < self.page_width() {
            self.first_page.append(value);
        } else {
            self.other_pages.append(value);
        }
    }

    #[inline]
    pub fn pop(&mut self) {
        if self.other_pages.is_empty() {
            self.first_page.pop()
        } else {
            self.other_pages.pop()
        }
    }
}

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
        self.vector.clear();
        self.num_entries = 0;
        self.start_ix = 0;
    }

    #[inline]
    pub fn reserve(&mut self, capacity: usize) {
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

    #[inline]
    pub fn set(&mut self, ix: usize, value: u64) {
        self.vector.set(self.internal_index(ix), value);
    }

    #[inline]
    pub fn get(&self, ix: usize) -> u64 {
        self.vector.get(self.internal_index(ix))
    }

    #[inline]
    pub fn push_front(&mut self, value: u64) {
        if self.num_entries == self.vector.len() {
            let capacity = Self::FACTOR * self.vector.len() as f64;
            let capacity = 1 + capacity as usize;
            self.reserve(capacity);
        }

        if self.start_ix == 0 {
            self.start_ix = self.vector.len() - 1;
        } else {
            self.start_ix -= 1;
        }

        self.num_entries += 1;

        self.set(0, value);
    }

    #[inline]
    pub fn push_back(&mut self, value: u64) {
        if self.num_entries == self.vector.len() {
            let capacity = Self::FACTOR * self.vector.len() as f64;
            let capacity = 1 + capacity as usize;
            self.reserve(capacity);
        }

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
        if ix < self.vector.len() - self.start_ix {
            self.start_ix + ix
        } else {
            ix - (self.vector.len() - self.start_ix)
        }
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

        let vector = vec![1, 2, 10, 120, 3];
        assert!(intvec.iter().eq(vector.into_iter()));
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

            let filled_correct = if filled_before > 0 {
                filled_after == filled_before - 1
            } else {
                filled_after == filled_before
            };

            filled_correct &&
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

    quickcheck! {
        fn prop_iter(vector: Vec<u64>) -> bool {
            let mut intvec = PackedIntVec::new();
            for &x in vector.iter() {
                intvec.append(x);
            }

            vector.into_iter().eq(intvec.iter())
        }
    }
}
