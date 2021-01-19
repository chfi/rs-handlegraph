use super::paged::PagedIntVec;
use super::vector::PackedIntVec;

use super::traits::*;

#[derive(Debug, Clone)]
pub struct RobustPagedIntVec {
    first_page: PackedIntVec,
    other_pages: PagedIntVec,
}

crate::impl_space_usage!(RobustPagedIntVec, [first_page, other_pages]);

impl RobustPagedIntVec {
    pub fn new(page_size: usize) -> Self {
        let width = 64 - page_size.leading_zeros() as usize;
        let first_page = PackedIntVec::new_with_width(width);
        let other_pages = PagedIntVec::new_with_width(page_size, width);
        Self {
            first_page,
            other_pages,
        }
    }

    pub fn resize(&mut self, new_size: usize) {
        if new_size > self.page_width() {
            self.first_page.resize(self.page_width());
            self.other_pages.resize(new_size - self.page_width());
        } else {
            self.first_page.resize(new_size);
            self.other_pages.clear();
        }
    }

    pub fn reserve(&mut self, capacity: usize) {
        if capacity > self.page_width() {
            self.first_page.reserve(self.page_width());
            self.other_pages.reserve(capacity - self.page_width());
        } else {
            self.first_page.reserve(capacity);
        }
    }

    #[inline]
    pub fn page_width(&self) -> usize {
        self.other_pages.page_width()
    }

    #[inline]
    pub fn page_size(&self) -> usize {
        self.other_pages.page_width()
    }

    #[inline]
    pub fn append_pages(&mut self, buf: &mut Vec<u64>, mut data: &[u64]) {
        if data.is_empty() {
            return;
        }

        if self.first_page.len() < self.page_width() {
            let first_page_slots = self.page_width() - self.first_page.len();
            let split_index = first_page_slots.min(data.len());
            let (page, rest) = data.split_at(split_index);
            self.first_page.append_slice(page);
            data = rest;
        }

        if !data.is_empty() {
            self.other_pages.append_pages(buf, data);
        }
    }

    pub fn print_diagnostics(&self) {
        println!("First page");
        print!(" -- ");
        self.first_page.print_diagnostics();
        println!("Other pages");
        self.other_pages.print_diagnostics();
    }
}

impl PackedCollection for RobustPagedIntVec {
    #[inline]
    fn len(&self) -> usize {
        self.first_page.len() + self.other_pages.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.first_page.is_empty() && self.other_pages.is_empty()
    }

    #[inline]
    fn clear(&mut self) {
        self.first_page.clear();
        self.other_pages.clear();
    }

    #[inline]
    fn set(&mut self, index: usize, value: u64) {
        if index < self.page_width() {
            self.first_page.set(index, value);
        } else {
            self.other_pages.set(index - self.page_width(), value);
        }
    }

    #[inline]
    fn get(&self, index: usize) -> u64 {
        if index < self.page_width() {
            self.first_page.get(index)
        } else {
            self.other_pages.get(index - self.page_width())
        }
    }

    #[inline]
    fn append(&mut self, value: u64) {
        if self.first_page.len() < self.page_width() {
            self.first_page.append(value);
        } else {
            self.other_pages.append(value);
        }
    }

    #[inline]
    fn pop(&mut self) {
        if self.other_pages.is_empty() {
            self.first_page.pop()
        } else {
            self.other_pages.pop()
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use quickcheck::{quickcheck, Arbitrary, Gen};

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
            paged.set(ix, value);

            let set_correct = paged.get(ix) == value;
            let len_correct = paged.len() == len_before;

            set_correct && len_correct
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
}
