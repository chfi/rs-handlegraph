use succinct::{IntVec, IntVecMut, IntVector};

use std::num::NonZeroUsize;

use super::traits::*;
use super::vector::PackedIntVec;

/// A packed integer vector divided into pages, but unlike
/// [`PagedIntVec`](super::paged::PagedIntVec), the page size is not a
/// hard limit, and `FlexPagedIntVec` is intended to store sequences
/// of elements of any length, as in
/// [`packedgraph::Sequences`](crate::packedgraph::sequences::Sequences).
///
/// To make this possible, `FlexPagedIntVec` supports adding entire
/// sequences of elements at once, and each such sequence is ensured
/// to be stored in the same page. When a page is longer than
/// `page_size_limit`, that page is full, and following sequences are
/// inserted to the next free page.
#[derive(Debug, Clone)]
pub struct FlexPagedIntVec {
    page_size_limit: usize,
    num_entries: usize,
    open_page: usize,
    first_page: PackedIntVec,
    other_pages: Vec<PackedIntVec>,
    initial_width: usize,
}

#[derive(Debug, Clone)]
pub struct Page {
    offset: usize,
    end: usize,
    // limit: usize,
    // end: Option<NonZeroUsize>,
    page: PackedIntVec,
}

impl Page {
    pub fn with_width(width: usize, offset: usize, length: usize) -> Self {
        let end = offset + length;
        let page = PackedIntVec::new_with_width(width);
        // let limit = length;
        Page {
            offset,
            end,
            // limit,
            page,
        }
    }

    pub fn new(offset: usize, length: usize) -> Self {
        Self::with_width(1, offset, length)
    }

    #[inline]
    fn len(&self) -> usize {
        self.page.len()
    }

    #[inline]
    fn closed(&self) -> bool {
        self.len() >= (self.end - self.offset)
    }

    #[inline]
    pub fn append(&mut self, value: u64) -> bool {
        self.page.append(value);
        !self.closed()
    }

    #[inline]
    pub fn append_slice(&mut self, items: &[u64]) -> bool {
        self.page.append_slice(items);
        !self.closed()
    }

    pub fn append_iter<I>(&mut self, width: usize, iter: I) -> bool
    where
        I: Iterator<Item = u64> + ExactSizeIterator,
    {
        self.page.append_iter(width, iter);
        !self.closed()
    }

    fn get(&self, index: usize) -> u64 {
        self.page.get(index)
    }

    fn set(&mut self, index: usize, value: u64) {
        self.page.set(index, value)
    }
}

pub struct FlexPaged_ {
    pub page_size_simit: usize,
    pub num_entries: usize,
    pub pages: Vec<Page>,
    pub open_page: usize,
}

crate::impl_space_usage!(FlexPagedIntVec, [first_page, other_pages]);

impl Default for FlexPagedIntVec {
    fn default() -> Self {
        // 8,388,608
        // 16,777,216
        let page_size_limit = 8_388_608;
        let initial_width = 2;
        let num_entries = 0;
        let open_page = 0;

        let first_page = PackedIntVec::new_with_width(initial_width);
        let other_pages = Vec::new();

        Self {
            page_size_limit,
            num_entries,
            open_page,
            first_page,
            other_pages,
            initial_width,
        }
    }
}

impl FlexPagedIntVec {
    pub fn with_page_size_limit(page_size_limit: usize) -> Self {
        Self {
            page_size_limit,
            ..Default::default()
        }
    }

    fn get_open_page(&self) -> &PackedIntVec {
        if self.open_page == 0 {
            &self.first_page
        } else {
            &self.other_pages[self.open_page - 1]
        }
    }

    fn get_open_page_mut(&mut self) -> &mut PackedIntVec {
        if self.open_page == 0 {
            &mut self.first_page
        } else {
            &mut self.other_pages[self.open_page - 1]
        }
    }

    fn reserve_open_page(&mut self, additional: usize) {
        let open_page = self.get_open_page_mut();
        let old_len = open_page.vector.len() as usize;
        let new_len = old_len + additional;
        open_page.reserve(new_len);
    }

    // fn resize_open_page(&mut self,
}
