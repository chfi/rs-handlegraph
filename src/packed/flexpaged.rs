use succinct::{IntVec, IntVecMut, IntVector};

use std::num::NonZeroUsize;

use super::traits::*;
use super::vector::PackedIntVec;

/// A packed integer vector divided into pages, but unlike
/// [`PagedIntVec`](super::paged::PagedIntVec), the page size is not a
/// hard limit, and `FlexPagedVec` is intended to store sequences
/// of elements of any length, as in
/// [`packedgraph::Sequences`](crate::packedgraph::sequences::Sequences).
///
/// To make this possible, `FlexPagedVec` supports adding entire
/// sequences of elements at once, and each such sequence is ensured
/// to be stored in the same page. When a page is longer than
/// `page_size_limit`, that page is full, and following sequences are
/// inserted to the next free page.
#[derive(Debug, Clone)]
pub struct FlexPagedVec {
    pub initial_width: usize,
    pub page_size_limit: usize,
    pub num_entries: usize,
    pub open_page: Page,
    pub closed_pages: Vec<Page>,
}

/// A "flexible" page used by [`FlexPagedVec`].
#[derive(Debug, Clone)]
pub struct Page {
    offset: usize,
    end: usize,
    vector: PackedIntVec,
}

crate::impl_space_usage!(Page, [vector]);

impl Page {
    pub fn with_width(width: usize, offset: usize, length: usize) -> Self {
        let end = offset + length;
        let vector = PackedIntVec::new_with_width(width);
        // let limit = length;
        Page {
            offset,
            end,
            // limit,
            vector,
        }
    }

    pub fn new(offset: usize, length: usize) -> Self {
        Self::with_width(1, offset, length)
    }

    #[inline]
    fn contains_index(&self, index: usize) -> bool {
        self.offset <= index && index < self.end
    }

    #[inline]
    fn len(&self) -> usize {
        self.vector.len()
    }

    #[inline]
    fn closed(&self) -> bool {
        self.len() >= (self.end - self.offset)
    }

    #[inline]
    pub fn append(&mut self, value: u64) -> bool {
        print!(" - Appending {:4} ... ", value);

        self.vector.append(value);

        if self.closed() {
            println!(" closing page");
        } else {
            println!(
                " page open, {} left",
                (self.end - self.offset) - self.len()
            );
        }

        self.closed()
    }

    #[inline]
    pub fn append_slice(&mut self, items: &[u64]) -> bool {
        print!(" - Appending slice of length {:4} ... ", items.len());

        self.vector.append_slice(items);

        if self.closed() {
            println!(" closing page");
        } else {
            println!(
                " page open, {} left",
                (self.end - self.offset) - self.len()
            );
        }

        self.closed()
    }

    pub fn append_iter<I>(&mut self, width: usize, iter: I) -> bool
    where
        I: Iterator<Item = u64> + ExactSizeIterator,
    {
        print!(
            " - Appending iter, width {:2}, length {:4} ... ",
            width,
            iter.size_hint().1.unwrap()
        );

        self.vector.append_iter(width, iter);

        if self.closed() {
            println!(" closing page");
        } else {
            println!(
                " page open, {} left",
                (self.end - self.offset) - self.len()
            );
        }

        self.closed()
    }

    #[inline]
    fn get(&self, index: usize) -> u64 {
        self.vector.get(index)
    }

    #[inline]
    fn set(&mut self, index: usize, value: u64) {
        self.vector.set(index, value)
    }
}

crate::impl_space_usage!(FlexPagedVec, [open_page, closed_pages]);

impl Default for FlexPagedVec {
    fn default() -> Self {
        let initial_width = 2;
        let page_size_limit = 8_388_608;
        Self::new(initial_width, page_size_limit)
    }
}

impl FlexPagedVec {
    fn new(initial_width: usize, page_size_limit: usize) -> Self {
        let num_entries = 0;

        let open_page = Page::with_width(initial_width, 0, page_size_limit);
        let closed_pages = Vec::new();

        Self {
            initial_width,
            page_size_limit,
            num_entries,
            open_page,
            closed_pages,
        }
    }

    #[inline]
    fn page_for(&self, index: usize) -> Option<&Page> {
        if index >= self.open_page.offset {
            Some(&self.open_page)
        } else {
            self.closed_pages.iter().find(|page| index < page.end)
        }
    }

    #[inline]
    fn page_mut_for(&mut self, index: usize) -> Option<&mut Page> {
        if index >= self.open_page.offset {
            Some(&mut self.open_page)
        } else {
            self.closed_pages.iter_mut().find(|page| index < page.end)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.num_entries
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<u64> {
        self.page_for(index)
            .map(|page| page.get(index - page.offset))
    }

    #[inline]
    pub fn set(&mut self, index: usize, value: u64) {
        let page = self.page_mut_for(index).unwrap();
        page.set(index - page.offset, value);
    }

    #[inline]
    fn close_page(&mut self) {
        let mut page = &mut self.open_page;
        page.end = page.offset + page.vector.len();

        let mut new_page = Page::with_width(
            self.initial_width,
            page.end,
            self.page_size_limit,
        );

        std::mem::swap(page, &mut new_page);
        self.closed_pages.push(new_page);
    }
}
