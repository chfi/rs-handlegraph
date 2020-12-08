use super::vector::PackedIntVec;

use super::traits::*;

#[derive(Debug, Clone)]
// pub struct PagedIntVec<Codec = XorCodec> {
pub struct PagedIntVec<Codec = DiffCodec> {
    // pub struct PagedIntVec<Codec = IdentityCodec> {
    pub page_size: usize,
    pub num_entries: usize,
    pub anchors: PackedIntVec,
    pub pages: Vec<PackedIntVec>,
    initial_width: usize,
    codec: Codec,
}

/// Abstraction of the method used by a [`PagedIntVec`] to pack
/// elements into a page, to reduce memory consumption.
pub trait PagedCodec {
    fn encode(value: u64, anchor: u64) -> u64;

    fn decode(value: u64, anchor: u64) -> u64;
}

#[derive(Debug, Clone, Default)]
pub struct XorCodec();

impl PagedCodec for XorCodec {
    #[inline]
    fn encode(value: u64, anchor: u64) -> u64 {
        if value == 0 {
            0
        } else {
            ((value ^ anchor) << 1) + 1
        }
    }

    #[inline]
    fn decode(diff: u64, anchor: u64) -> u64 {
        if diff == 0 {
            0
        } else {
            (diff >> 1) ^ anchor
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiffCodec();

impl PagedCodec for DiffCodec {
    #[inline]
    fn encode(value: u64, anchor: u64) -> u64 {
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
    fn decode(diff: u64, anchor: u64) -> u64 {
        if diff == 0 {
            0
        } else if diff % 5 == 0 {
            anchor - diff / 5
        } else {
            anchor + diff - diff / 5 - 1
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IdentityCodec();

impl PagedCodec for IdentityCodec {
    #[inline]
    fn encode(value: u64, _anchor: u64) -> u64 {
        value
    }

    #[inline]
    fn decode(value: u64, _anchor: u64) -> u64 {
        value
    }
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
        let initial_width = 1;
        PagedIntVec {
            page_size,
            num_entries,
            anchors,
            pages,
            initial_width,
            codec: Default::default(),
        }
    }

    pub fn new_with_width(page_size: usize, initial_width: usize) -> Self {
        let num_entries = 0;
        let pages = Vec::new();
        let anchors = Default::default();
        PagedIntVec {
            page_size,
            num_entries,
            anchors,
            pages,
            initial_width,
            codec: Default::default(),
        }
    }

    pub fn resize_with_width(&mut self, new_size: usize, width: usize) {
        #[allow(clippy::comparison_chain)]
        if new_size < self.num_entries {
            let num_pages = if new_size == 0 {
                0
            } else {
                (new_size - 1) / self.page_size + 1
            };

            self.anchors.resize(num_pages);
            self.pages
                .resize_with(num_pages, || PackedIntVec::new_with_width(width));
        } else if new_size > self.num_entries {
            self.reserve(new_size);
        }

        self.num_entries = new_size;
    }

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
    pub(super) fn page_width(&self) -> usize {
        self.page_size
    }

    pub fn save_diagnostics<W: std::io::Write>(
        &self,
        mut w: W,
    ) -> std::io::Result<()> {
        writeln!(&mut w, "# Elements {:6}", self.num_entries)?;
        writeln!(&mut w, "# Pages    {:6}", self.pages.len())?;

        writeln!(
            &mut w,
            "{:<7},{:<5},{:<6},{:<6},{:<6}",
            "PageIx", "Width", "Min", "Median", "Max"
        )?;

        for (page_ix, page) in self.pages.iter().enumerate() {
            let mut min = std::u64::MAX;
            let mut max = 0u64;
            let mut median = 0u64;

            for (i, v) in page.iter().enumerate() {
                min = min.min(v);
                max = max.max(v);
                if i == page.len() / 2 {
                    median = v;
                }
            }

            writeln!(
                &mut w,
                "{:<7},{:<5},{:<6},{:<6},{:<6}",
                page_ix,
                page.width(),
                min,
                median,
                max
            )?;
        }

        Ok(())
    }

    pub fn print_diagnostics(&self) {
        println!(
            "Elements {:6}\tPage size {:4}\tPages {:6}",
            self.num_entries,
            self.page_size,
            self.pages.len()
        );
        println!(
            "{:>7}\t{:>5}\t{:>6}\t{:>6}",
            "Page Ix", "Width", "Min", "Max"
        );
        for (page_ix, page) in self.pages.iter().enumerate() {
            let mut min = std::u64::MAX;
            let mut max = 0u64;

            for v in page.iter() {
                min = min.min(v);
                max = max.max(v);
            }

            println!(
                "{:>7}\t{:>5}\t{:>6}\t{:>6}",
                page_ix,
                page.width(),
                min,
                max
            );
        }
    }
}

impl<T: PagedCodec> PagedIntVec<T> {
    /// Fills the last page in this [`PagedIntVec`] using the provided
    /// slice, without adding a new page. Returns `None` if the `self`
    /// is empty, the last page is full, or if `data` is empty.
    /// Otherwise, returns the remainder of the slice that wasn't
    /// added to the page.
    #[inline]
    fn fill_last_page<'a>(&mut self, data: &'a [u64]) -> Option<&'a [u64]> {
        let total_slots = self.pages.len() * self.page_size;
        let last_page_slots = total_slots - self.num_entries;
        if last_page_slots == 0 || data.is_empty() || self.anchors.is_empty() {
            return None;
        }

        let split_index = last_page_slots.min(data.len());

        let (page, rest) = data.split_at(split_index);

        let anchor = self.anchors.get(self.pages.len() - 1);

        let buf = page
            .iter()
            .copied()
            .map(|v| T::encode(v, anchor))
            .collect::<Vec<_>>();

        let width = buf.iter().copied().max().map(super::width_for)?;

        let last_page = self.pages.last_mut()?;

        last_page.append_iter(width, buf.into_iter());

        self.num_entries += page.len();

        Some(rest)
    }

    /// Append a new page containing the values in `data`. Returns the
    /// subslice of elements that didn't fit in the page size, or
    /// `None` if the last page in the vector was not full, or `data`
    /// was empty.
    #[inline]
    fn append_page<'a>(&mut self, data: &'a [u64]) -> Option<&'a [u64]> {
        let total_slots = self.pages.len() * self.page_size;
        let last_page_slots = total_slots - self.num_entries;

        if self.num_entries != total_slots || data.is_empty() {
            return None;
        }

        let split_index = self.page_size.min(data.len());

        let (page, rest) = data.split_at(split_index);

        let anchor = page.iter().copied().filter(|&x| x != 0).min()?;

        let width = page.iter().copied().max().map(super::width_for)?;

        let mut new_page =
            PackedIntVec::with_width_and_capacity(width, self.page_size);

        for &value in page.iter() {
            let encoded = T::encode(value, anchor);
            new_page.append(encoded);
        }

        self.anchors.append(anchor);
        self.pages.push(new_page);

        self.num_entries += split_index;

        Some(rest)
    }

    #[inline]
    fn append_pages<I>(&mut self, mut iter: I)
    where
        I: Iterator<Item = u64> + ExactSizeIterator,
    {
        let mut buffer: Vec<u64> = Vec::with_capacity(self.page_size);

        loop {
            if iter.len() == 0 {
                break;
            }

            buffer.clear();
            buffer.extend(iter.by_ref().take(self.page_size));

            let (anchor, max) = buffer
                .iter()
                .filter(|&&x| x != 0)
                .fold((std::u64::MAX, 0u64), |(a, m), &x| (a.min(x), m.max(x)));

            let width = super::width_for(T::encode(max, anchor));

            let mut new_page = PackedIntVec::new_with_width(width);
            new_page.resize(self.page_size);
            new_page.append_iter(
                width,
                buffer.iter().map(|&x| T::encode(x, anchor)),
            );
            self.anchors.append(anchor);
            self.pages.push(new_page);
        }
    }

    #[inline]
    pub fn append_iter<I>(&mut self, mut iter: I)
    where
        I: Iterator<Item = u64> + ExactSizeIterator,
    {
        let total_slots = self.pages.len() * self.page_size;

        if self.num_entries == total_slots {
            self.append_pages(iter);
        } else {
            let empty_last_page = total_slots - self.num_entries;
            if let Some(last_page) = self.pages.last_mut() {
                let width = last_page.width();
                last_page
                    .append_iter(width, iter.by_ref().take(empty_last_page));
                self.append_pages(iter);
            }
        }
    }
}

impl<T: PagedCodec> PackedCollection for PagedIntVec<T> {
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
            .set(index % self.page_size, T::encode(value, anchor));
    }

    #[inline]
    fn get(&self, index: usize) -> u64 {
        assert!(index < self.num_entries);
        let page_ix = index / self.page_size;
        T::decode(
            self.pages[page_ix].get(index % self.page_size),
            self.anchors.get(page_ix),
        )
    }

    #[inline]
    fn append(&mut self, value: u64) {
        if self.num_entries == self.pages.len() * self.page_size {
            let mut new_page = PackedIntVec::new_with_width(self.initial_width);
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

#[cfg(test)]
mod tests {

    use super::*;
    use quickcheck::{quickcheck, Arbitrary, Gen};

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
