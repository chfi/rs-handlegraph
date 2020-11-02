use super::paged::PagedIntVec;
use super::vector::PackedIntVec;

use super::traits::*;

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
    pub(super) fn resize(&mut self, new_size: usize) {
        if new_size > self.page_width() {
            self.first_page.resize(self.page_width());
            self.other_pages.resize(new_size - self.page_width());
        } else {
            self.first_page.resize(new_size);
            self.other_pages.clear();
        }
    }

    #[inline]
    pub(super) fn reserve(&mut self, capacity: usize) {
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
