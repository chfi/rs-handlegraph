use succinct::{IntVec, IntVecMut, IntVector};

mod traits;

pub mod vector;

pub use self::traits::*;

pub use self::vector::PackedIntVec;

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
        #[allow(clippy::comparison_chain)]
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

    #[allow(clippy::wrong_self_convention)]
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

    #[allow(clippy::wrong_self_convention)]
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

    pub fn iter(&self) -> PackedDequeIter<'_> {
        PackedDequeIter::new(self)
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

pub struct PackedDequeIter<'a> {
    deque: &'a PackedDeque,
    index: usize,
    len: usize,
}

impl<'a> PackedDequeIter<'a> {
    fn new(deque: &'a PackedDeque) -> Self {
        Self {
            deque,
            index: 0,
            len: deque.len(),
        }
    }
}

impl<'a> Iterator for PackedDequeIter<'a> {
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

impl<'a> std::iter::FusedIterator for PackedDequeIter<'a> {}

impl std::iter::FromIterator<u64> for PackedDeque {
    fn from_iter<I: IntoIterator<Item = u64>>(iter: I) -> Self {
        let mut deque = PackedDeque::new();
        iter.into_iter().for_each(|v| deque.push_back(v));
        deque
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

impl Arbitrary for PagedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> PagedIntVec {
        let mut paged = PagedIntVec::new(64);
        let u64_vec: Vec<u64> = Vec::arbitrary(g);

        for v in u64_vec {
            paged.append(v);
        }
        paged
    }
}

impl Arbitrary for RobustPagedIntVec {
    fn arbitrary<G: Gen>(g: &mut G) -> RobustPagedIntVec {
        let only_first = bool::arbitrary(g);

        let page_pow = u32::arbitrary(g) % 4;
        let page_size = 16 << page_pow;

        assert!(page_size % 2 == 0 && page_size >= 16 && page_size <= 256);
        let mut paged = RobustPagedIntVec::new(page_size);
        let mut values: Vec<u64> = Vec::arbitrary(g);

        if !only_first {
            while values.len() < page_size {
                let v = u64::arbitrary(g);
                values.push(v);
            }
        }

        values.into_iter().for_each(|v| paged.append(v));

        paged
    }
}

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

#[cfg(test)]
mod tests {

    use quickcheck::quickcheck;

    use super::*;

    #[test]
    fn test_intvec_append() {
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
        fn prop_intvec_append(intvec: PackedIntVec, value: u64) -> bool {
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
        fn prop_intvec_pop(intvec: PackedIntVec) -> bool {
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
        fn prop_intvec_get(vector: Vec<u64>) -> bool {
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
        fn prop_intvec_iter(vector: Vec<u64>) -> bool {
            let mut intvec = PackedIntVec::new();
            for &x in vector.iter() {
                intvec.append(x);
            }

            vector.into_iter().eq(intvec.iter())
        }
    }

    quickcheck! {
        fn prop_paged_append(paged: PagedIntVec, value: u64) -> bool {
            let mut paged = paged;

            let entries_before = paged.len();

            paged.append(value);

            let entries_correct = paged.len() == entries_before + 1;
            let last_val_correct = paged.get(paged.len() - 1) == value;

            entries_correct && last_val_correct
        }
    }

    quickcheck! {
        fn prop_paged_set(paged: PagedIntVec, ix: usize, value: u64) -> bool {
            let mut paged = paged;
            if paged.len() == 0 {
                return true;
            }
            let ix = ix % paged.len();

            let len_before = paged.len();
            let pages_before = paged.pages.len();
            paged.set(ix, value);

            let set_correct = paged.get(ix) == value;
            let len_correct = paged.len() == len_before;
            let pages_correct = paged.pages.len() == pages_before;

            set_correct && len_correct && pages_correct
        }
    }

    quickcheck! {
        fn prop_paged_pop(paged: PagedIntVec) -> bool {
            let mut paged = paged;

            let len_before = paged.len();

            paged.pop();

            let len_correct = if len_before == 0 {
                paged.len() == 0
            } else {
                paged.len() == len_before - 1
            };

            len_correct
        }
    }

    quickcheck! {
        fn prop_robust_append(paged: RobustPagedIntVec, value: u64) -> bool {
            let mut paged = paged;

            let entries_before = paged.len();

            paged.append(value);

            let entries_correct = paged.len() == entries_before + 1;
            let last_val_correct = paged.get(paged.len() - 1) == value;

            entries_correct && last_val_correct
        }
    }

    quickcheck! {
        fn prop_robust_set(paged: RobustPagedIntVec, ix: usize, value: u64) -> bool {
            let mut paged = paged;
            if paged.len() == 0 {
                return true;
            }
            let ix = ix % paged.len();

            let len_before = paged.len();
            let first_len_before = paged.first_page.len();
            let pages_before = paged.other_pages.pages.len();
            paged.set(ix, value);

            let set_correct = paged.get(ix) == value;
            let len_correct = paged.len() == len_before;
            let first_len_correct = paged.first_page.len() == first_len_before;
            let pages_correct = paged.other_pages.pages.len() == pages_before;

            set_correct && len_correct && first_len_correct && pages_correct
        }
    }

    quickcheck! {
        fn prop_robust_pop(paged: RobustPagedIntVec) -> bool {
            let mut paged = paged;

            let len_before = paged.len();

            paged.pop();

            if len_before == 0 {
                paged.len() == 0
            } else {
                paged.len() == len_before - 1
            }
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
