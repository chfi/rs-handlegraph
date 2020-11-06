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

/// A packed list that supports mutation by removing records, while
/// updating the links in the list.
pub trait PackedListMut: PackedList {
    type ListLink: PartialEq + Copy;

    fn get_record_link(record: &Self::ListRecord) -> Self::ListLink;

    /// Remove the list record at the given pointer, if it exists.
    /// Returns the removed record's next pointer.
    fn remove_at_pointer(
        &mut self,
        ptr: Self::ListPtr,
    ) -> Option<Self::ListLink>;

    /// Remove the list record after the given pointer, if it exists.
    /// Should update the provided record's next pointer accordingly.
    fn remove_next(&mut self, ptr: Self::ListPtr) -> Option<()>;
}

/// An iterator through linked lists represented using PackedList
pub struct Iter<'a, T: PackedList> {
    list: &'a T,
    head_ptr: T::ListPtr,
    tail_ptr: T::ListPtr,
    finished: bool,
}

pub struct IterMut<'a, T: PackedList> {
    list: &'a mut T,
    head_ptr: T::ListPtr,
    tail_ptr: T::ListPtr,
    finished: bool,
}

fn find_record_with_prev_ix<'a, 'b, T, P>(
    iter: &'a mut IterMut<'b, T>,
    p: P,
) -> Option<(T::ListPtr, T::ListPtr)>
where
    T: PackedList,
    P: Fn(T::ListPtr, T::ListRecord) -> bool,
{
    let (prev_ptr, rec_ptr, _) = iter
        .scan(T::ListPtr::null(), |prev_ptr, (rec_ptr, record)| {
            let old_prev_ptr = *prev_ptr;
            *prev_ptr = rec_ptr;
            Some((old_prev_ptr, rec_ptr, record))
        })
        .find(|&(_, ptr, rec)| p(ptr, rec))?;

    Some((prev_ptr, rec_ptr))
}

impl<'a, T: PackedListMut> IterMut<'a, T> {
    pub fn remove_record_with<P>(&mut self, p: P) -> Option<T::ListLink>
    where
        P: Fn(T::ListPtr, T::ListRecord) -> bool,
    {
        let head = self.head_ptr;
        let tail = self.tail_ptr;

        let (prev_ptr, rec_ptr) = find_record_with_prev_ix(self, p)?;

        if prev_ptr.is_null() {
            assert!(head == rec_ptr);
            let next = self.list.remove_at_pointer(head)?;
            Some(next)
        } else {
            let head_record = self.list.get_record(head)?;
            let head_link = T::get_record_link(&head_record);

            if tail == rec_ptr {
                self.tail_ptr = T::ListPtr::null();
            }
            self.list.remove_next(rec_ptr);
            Some(head_link)
        }
    }
}

macro_rules! list_iter_impls {
    ($iter:ty, $list:ty, $trait:path) => {
        impl<'a, T: $trait> $iter {
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
    };
}

macro_rules! list_iter_impl_iter_traits {
    ($iter:ty) => {
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

list_iter_impls!(Iter<'a, T>, &'a T, PackedList);
list_iter_impl_iter_traits!(Iter<'a, T>);

list_iter_impls!(IterMut<'a, T>, &'a mut T, PackedList);
list_iter_impl_iter_traits!(IterMut<'a, T>);
