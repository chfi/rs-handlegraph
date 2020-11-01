use crate::{
    handle::{Direction, Handle, NodeId},
    packed::*,
};

use std::num::NonZeroUsize;

/// The identifier and index for all node-related records in the PackedGraph.
///
/// This is used whenever we have some type of record, in one or more
/// packed collections, such that each node in the graph has exactly
/// one such record.
///
/// This index is 1-based, with 0 denoting missing data, the empty
/// record, or the empty list, depending on the context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeRecordId(Option<NonZeroUsize>);

impl NodeRecordId {
    /// Construct a `NodeRecordId` from a 0-based index, shifting
    /// it into the 1-based index. The resulting `NodeRecordId` will
    /// never be the null identifier.
    #[inline]
    pub(super) fn from_zero_based<I: Into<usize>>(ix: I) -> Self {
        Self(NonZeroUsize::new(ix.into() + 1))
    }

    /// Construct a `NodeRecordId` from a 0-based index for records
    /// that are `width` elements long, adjusting for the record width
    /// and shifting it into a 1-based index. The resulting
    /// `NodeRecordId` will never be the null identifier.
    #[inline]
    pub(super) fn from_record_ix<I: Into<usize>>(ix: I, width: usize) -> Self {
        Self(NonZeroUsize::new((ix.into() / width) + 1))
    }

    /// Construct a `NodeRecordId` from a 1-based index. If the input
    /// is zero, the resulting `NodeRecordId` will be the null
    /// identifier.
    #[inline]
    pub(super) fn from_one_based<I: Into<usize>>(ix: I) -> Self {
        Self(NonZeroUsize::new(ix.into()))
    }

    /// Transform the `NodeRecordId` into a 0-based index that can be
    /// used to retrieve the element corresponding to this identifier
    /// from a collection.
    ///
    /// Shifts the wrapped `NonZeroUsize` down by 1, and returns
    /// `None` if the identifier is null.
    #[inline]
    pub(super) fn to_zero_based(self) -> Option<usize> {
        self.0.map(|u| u.get() - 1)
    }

    /// Transform the `NodeRecordId` into a 0-based index that can be
    /// used to retrieve a `width` elements long record in a
    /// collection at the index corresponding to this identifier.
    ///
    /// Shifts the wrapped `NonZeroUsize` down by 1 before adjusting
    /// for the record width, and returns `None` if the identifier is
    /// null.
    #[inline]
    pub(super) fn to_record_ix(self, width: usize) -> Option<usize> {
        self.0.map(|u| (u.get() - 1) * width)
    }

    /// Build a `NodeRecordId` from a `u64` that was stored in a collection.
    #[inline]
    pub(super) fn from_vector_value(v: u64) -> Self {
        Self::from_zero_based(v as usize)
    }

    /// Transform the `NodeRecordId` into a `u64` that can be stored
    /// in a collection.
    #[inline]
    pub(super) fn to_vector_value(self) -> u64 {
        match self.0 {
            None => 0,
            Some(x) => x.get() as u64,
        }
    }

    /// `true` if this identifier is the null identifier.
    #[inline]
    pub(super) fn is_null(&self) -> bool {
        self.0.is_none()
    }

    /// Returns the null identifier.
    #[inline]
    pub(super) fn null() -> Self {
        Self(None)
    }
}

pub trait RecordIndex: Copy {
    const RECORD_WIDTH: usize;

    fn from_node_record_id(id: NodeRecordId) -> Option<Self>;

    fn to_node_record_id(self) -> NodeRecordId;

    fn to_vector_index(self, offset: usize) -> usize;
}
