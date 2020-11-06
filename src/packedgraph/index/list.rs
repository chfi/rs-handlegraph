use std::num::NonZeroUsize;

use crate::packed::traits::PackedElement;

use super::OneBasedIndex;

/// A collection of linked lists implemented using one or more
/// vector-like collections.
pub trait PackedList {
    type ListPtr: PartialEq + OneBasedIndex;
    type ListRecord: Copy;

    /// Extract the pointer for a record
    fn next_pointer(rec: &Self::ListRecord) -> Self::ListPtr;

    /// Retrieve the record for the given pointer, if the pointer is
    /// not the empty list
    fn get_record(&self, ptr: Self::ListPtr) -> Option<Self::ListRecord>;

    /// Return the record that comes after the provided record, if
    /// we're not already at the end of the list
    #[inline]
    fn next_record(&self, rec: &Self::ListRecord) -> Option<Self::ListRecord> {
        self.get_record(Self::next_pointer(rec))
    }
}

/// A collection of doubly linked lists implemented using packed vectors.
pub trait PackedDoubleList: PackedList {
    fn prev_pointer(rec: &Self::ListRecord) -> Self::ListPtr;

    #[inline]
    fn prev_record(&self, rec: &Self::ListRecord) -> Option<Self::ListRecord> {
        self.get_record(Self::prev_pointer(rec))
    }
}

macro_rules! list_iter_impls {
    ($iter:ty, $list:ty) => {
        impl<'a, T: PackedList> $iter {
            pub fn new(list: $list, head_ptr: T::ListPtr) -> Self {
                let tail_ptr = T::ListPtr::null();
                Self {
                    list,
                    head_ptr,
                    tail_ptr,
                    finished: false,
                }
            }

            pub fn new_double(
                list: $list,
                head_ptr: T::ListPtr,
                tail_ptr: T::ListPtr,
            ) -> Self {
                Self {
                    list,
                    head_ptr,
                    tail_ptr,
                    finished: false,
                }
            }
        }

        impl<'a, T: PackedList> Iterator for $iter {
            type Item = (T::ListPtr, T::ListRecord);

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                if self.finished {
                    return None;
                }
                if self.head_ptr == self.tail_ptr {
                    self.finished = true;
                }
                let record = self.list.get_record(self.head_ptr)?;
                let this_ptr = self.head_ptr;
                self.head_ptr = T::next_pointer(&record);
                Some((this_ptr, record))
            }
        }

        impl<'a, T: PackedDoubleList> DoubleEndedIterator for $iter {
            #[inline]
            fn next_back(&mut self) -> Option<Self::Item> {
                if self.finished {
                    return None;
                }
                if self.head_ptr == self.tail_ptr {
                    self.finished = true;
                }

                let record = self.list.get_record(self.tail_ptr)?;
                let this_ptr = self.tail_ptr;
                self.tail_ptr = T::prev_pointer(&record);
                Some((this_ptr, record))
            }
        }
    };
}

/// An iterator through linked lists represented using PackedList
pub struct Iter<'a, T: PackedList> {
    list: &'a T,
    head_ptr: T::ListPtr,
    tail_ptr: T::ListPtr,
    finished: bool,
}

list_iter_impls!(Iter<'a, T>, &'a T);
