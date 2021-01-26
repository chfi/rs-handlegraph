use std::num::NonZeroUsize;

use crate::packed::traits::PackedElement;

pub mod list;
pub use list::*;

/// An index that is 1-based, and uses 0 to denote missing data/the
/// empty record/the empty list.
///
/// Can be constructed from zero-based indices (e.g. when using the
/// length of a collection to produce the new index, and the
/// collection is empty), or unwrapped 1-based indices.
pub trait OneBasedIndex: Copy + Sized {
    /// Construct a 1-based index from a 0-based index, shifting
    /// it into the 1-based index. The resulting index will
    /// never be the null identifier.
    fn from_zero_based<I: Into<usize>>(ix: I) -> Self;

    /// Construct a 1-based index from a 0-based index for records
    /// that are `width` elements long, adjusting for the record width
    /// and shifting it into a 1-based index. The resulting index will
    /// never be the null identifier.
    fn from_record_start<I: Into<usize>>(ix: I, width: usize) -> Self;

    /// Construct a 1-based index from a 1-based index. If the input
    /// is zero, the resulting index will be the null
    /// identifier.
    fn from_one_based<I: Into<usize>>(ix: I) -> Self;

    /// Transform the 1-based index into a 0-based index that can be
    /// used to retrieve the element corresponding to this identifier
    /// from a collection.
    ///
    /// Returns `None` if the index is the null index.
    fn to_zero_based(self) -> Option<usize>;

    /// Transform the 1-based index into a 0-based index that can be
    /// used to retrieve a `width` elements long record in a
    /// collection at the index corresponding to this identifier.
    ///
    /// Returns `None` if the index is the null index.
    fn to_record_start(self, width: usize) -> Option<usize>;

    /// Transform the 1-based index into a 0-based index that can be
    /// used to retrieve the `ix`th field in a `width` elements long
    /// record in a collection at the index corresponding to this
    /// identifier.
    ///
    /// Returns `None` if the index is the null index.
    fn to_record_ix(self, width: usize, ix: usize) -> Option<usize>;

    /// Build a 1-based index from a `u64` that was stored in a collection.
    fn from_vector_value(v: u64) -> Self;

    /// Transform the 1-based index into a `u64` that can be stored in
    /// a collection.
    fn to_vector_value(self) -> u64;

    /// `true` if this identifier is the null identifier.
    fn is_null(&self) -> bool;

    /// Returns the null identifier.
    fn null() -> Self;
}

/// Any `OneBasedIndex` can be stored in any kind of packed collection.
impl<T: OneBasedIndex> PackedElement for T {
    #[inline]
    fn unpack(v: u64) -> Self {
        Self::from_vector_value(v)
    }

    #[inline]
    fn pack(self) -> u64 {
        self.to_vector_value()
    }
}

/// A 0-based index into a collection that nominally uses 1-based
/// indexing, with a zero denoting a missing record.
pub trait RecordIndex: Copy {
    const RECORD_WIDTH: usize;

    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self>;

    fn to_one_based_ix<I: OneBasedIndex>(self) -> I;

    fn record_ix(self, offset: usize) -> usize;

    /// Convenience method for getting the index of the first field at
    /// this record index
    #[inline]
    fn at_0(self) -> usize {
        self.record_ix(0)
    }
}

/// The identifier and index for all node-related records in the
/// PackedGraph.
///
/// This is used whenever we have some type of record, in one or more
/// packed collections, such that each node in the graph has exactly
/// one such record.
///
/// This index is 1-based, with 0 denoting missing data, the empty
/// record, or the empty list, depending on the context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeRecordId(Option<NonZeroUsize>);

#[macro_export]
macro_rules! impl_one_based_index {
    ($for:ty) => {
        impl OneBasedIndex for $for {
            #[inline]
            fn from_zero_based<I: Into<usize>>(ix: I) -> Self {
                Self(NonZeroUsize::new(ix.into() + 1))
            }
            #[inline]
            fn from_record_start<I: Into<usize>>(ix: I, width: usize) -> Self {
                Self(NonZeroUsize::new((ix.into() / width) + 1))
            }

            #[inline]
            fn from_one_based<I: Into<usize>>(ix: I) -> Self {
                Self(NonZeroUsize::new(ix.into()))
            }

            #[inline]
            fn to_zero_based(self) -> Option<usize> {
                self.0.map(|u| u.get() - 1)
            }

            #[inline]
            fn to_record_start(self, width: usize) -> Option<usize> {
                self.0.map(|u| (u.get() - 1) * width)
            }

            #[inline]
            fn to_record_ix(self, width: usize, ix: usize) -> Option<usize> {
                self.0.map(|u| ((u.get() - 1) * width) + ix)
            }

            #[inline]
            fn from_vector_value(v: u64) -> Self {
                Self(NonZeroUsize::new(v as usize))
            }

            #[inline]
            fn to_vector_value(self) -> u64 {
                match self.0 {
                    None => 0,
                    Some(x) => x.get() as u64,
                }
            }

            #[inline]
            fn is_null(&self) -> bool {
                self.0.is_none()
            }

            #[inline]
            fn null() -> Self {
                Self(None)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_one_based_index_default {
    ($index:ty) => {
        impl Default for $index {
            #[inline]
            fn default() -> Self {
                Self::null()
            }
        }
    };
}

#[macro_export]
macro_rules! impl_space_usage_stack_newtype {
    ($type:ty) => {
        impl succinct::SpaceUsage for $type {
            #[inline]
            fn is_stack_only() -> bool {
                true
            }
            #[inline]
            fn heap_bytes(&self) -> usize {
                0
            }
        }
    };
}

impl_one_based_index!(NodeRecordId);
impl_space_usage_stack_newtype!(NodeRecordId);
