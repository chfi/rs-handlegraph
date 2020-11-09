use succinct::SpaceUsage;

use super::vector::PackedIntVec;

use super::traits::*;

use quickcheck::{Arbitrary, Gen};

#[derive(Debug, Clone)]
pub struct PagedIntVec {
    page_size: usize,
    num_entries: usize,
    anchors: PackedIntVec,
    pages: Vec<PackedIntVec>,
}

crate::impl_space_usage!(PagedIntVec, [anchors, pages]);

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

    #[allow(dead_code)]
    pub(super) fn resize(&mut self, new_size: usize) {
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

    #[allow(dead_code)]
    pub(super) fn reserve(&mut self, new_size: usize) {
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
    pub(super) fn page_width(&self) -> usize {
        self.page_size
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

impl PackedCollection for PagedIntVec {
    #[inline]
    fn len(&self) -> usize {
        self.num_entries
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    fn clear(&mut self) {
        self.pages.clear();
        self.anchors.clear();
        self.num_entries = 0;
    }

    #[inline]
    fn set(&mut self, index: usize, value: u64) {
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
    fn get(&self, index: usize) -> u64 {
        assert!(index < self.num_entries);
        let page_ix = index / self.page_size;
        Self::from_diff(
            self.pages[page_ix].get(index % self.page_size),
            self.anchors.get(page_ix),
        )
    }

    #[inline]
    fn append(&mut self, value: u64) {
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
    fn pop(&mut self) {
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

#[cfg(test)]
mod tests {

    use quickcheck::quickcheck;

    use super::*;

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
}
