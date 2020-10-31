#![allow(dead_code)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_imports)]

use gfa::{
    gfa::{Link, Orientation, Segment, GFA},
    optfields::OptFields,
};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::HandleGraph,
    mutablehandlegraph::MutableHandleGraph,
    packed::*,
};

use std::num::NonZeroUsize;

use super::graph::{NARROW_PAGE_WIDTH, WIDE_PAGE_WIDTH};

/// The index for an edge record. Valid indices are natural numbers
/// starting from 1, each denoting a *record*. An edge list index of
/// zero denotes a lack of an edge, or the empty edge list.
///
/// As zero is used to represent no edge/the empty edge list,
/// `Option<NonZeroUsize>` is a natural fit for representing this.
#[derive(Debug, Clone, Copy)]
pub struct EdgeListIx(Option<NonZeroUsize>);

impl EdgeListIx {
    /// Create a new `EdgeListIx` by wrapping a `usize`. Should only
    /// be used in the PackedGraph edge list internals.
    ///
    /// If `x` is zero, the result will be `EdgeListIx(None)`.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    /// Unwrap the `EdgeListIx` into a `u64` for use in a packed
    /// vector. Should never be used other than when setting the
    /// `next` field of an edge list record.
    #[inline]
    fn as_vec_value(&self) -> u64 {
        match self.0 {
            None => 0,
            Some(v) => v.get() as u64,
        }
    }

    /// Wrap a `u64`, e.g. a value from a packed vector element, as an
    /// `EdgeListIx`.
    #[inline]
    fn from_vec_value(x: u64) -> Self {
        Self(NonZeroUsize::new(x as usize))
    }

    /// Transforms the `EdgeListIx` into an index that can be used to
    /// get the first element of a record from an edge list vector.
    /// Returns None if the `EdgeListIx` is None.
    ///
    /// `x -> (x - 1) * 2`
    #[inline]
    pub fn as_vec_ix(&self) -> Option<EdgeVecIx> {
        let x = self.0?.get();
        Some(EdgeVecIx((x - 1) * 2))
    }
}

/// The index into the underlying packed vector that is used to
/// represent the edge lists.

/// Each edge list record takes up two elements, so an `EdgeVecIx` is
/// always even. They also start from zero, so there's an offset by one
/// compared to `EdgeListIx`, besides the record size.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct EdgeVecIx(usize);

impl EdgeVecIx {
    /// Create a new `EdgeVecIx` by wrapping a `usize`. Should only be
    /// used in the PackedGraph edge list internals.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }

    /// Transforms the `EdgeVecIx` into an index that denotes a record
    /// in the edge list. The resulting `EdgeListIx` will always
    /// contain a value, never `None`.
    ///
    /// `x -> (x / 2) + 1`
    #[inline]
    pub fn as_list_ix(&self) -> EdgeListIx {
        EdgeListIx::new((self.0 / 2) + 1)
    }

    #[inline]
    fn handle_ix(&self) -> usize {
        self.0
    }

    #[inline]
    fn next_ix(&self) -> usize {
        self.0 + 1
    }
}

/// A packed vector containing the edges of the graph encoded as
/// multiple linked lists.
///
/// Each record takes up two elements, and is of the form `(Handle,
/// EdgeListIx)`, where the `Handle` is the target of the edge, and
/// the `EdgeListIx` is a pointer to the next edge record in the list.
///
/// Outwardly this is indexed using `EdgeListIx`, and the parts of a
/// record is indexed using `EdgeVecIx`.
#[derive(Debug, Clone)]
pub struct EdgeLists {
    record_vec: PagedIntVec,
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            record_vec: PagedIntVec::new(WIDE_PAGE_WIDTH),
        }
    }
}

pub type EdgeRecord = (Handle, EdgeListIx);

impl EdgeLists {
    const RECORD_SIZE: usize = 2;

    /// Returns the number of edge records -- *not* the number of elements.
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.record_vec.len() / Self::RECORD_SIZE
    }

    /// Get the handle for the record at the index, if the index is not null
    #[inline]
    fn get_handle(&self, ix: EdgeListIx) -> Option<Handle> {
        let h_ix = ix.as_vec_ix()?.handle_ix();
        let handle = Handle::from_integer(self.record_vec.get(h_ix));
        Some(handle)
    }

    /// Get the pointer to the following record, for the record at the
    /// index, if the index is not null. Will return `Some` even if
    /// the pointer is null, but the contained `EdgeListIx` will
    /// instead be null.
    #[inline]
    fn get_next(&self, ix: EdgeListIx) -> Option<EdgeListIx> {
        let n_ix = ix.as_vec_ix()?.next_ix();
        let next = EdgeListIx::from_vec_value(self.record_vec.get(n_ix));
        Some(next)
    }

    /// Get the handle and next pointer for the given record index.
    #[inline]
    fn get_record(&self, ix: EdgeListIx) -> Option<EdgeRecord> {
        let handle = self.get_handle(ix)?;
        let next = self.get_next(ix)?;
        Some((handle, next))
    }

    /// Create a new *empty* record and return its `EdgeListIx`.
    fn append_record(&mut self) -> EdgeListIx {
        let vec_ix = EdgeVecIx::new(self.record_vec.len());
        self.record_vec.append(0);
        self.record_vec.append(0);
        vec_ix.as_list_ix()
    }

    /// Update the `Handle` and pointer to the next `EdgeListIx` in
    /// the record at the provided `EdgeListIx`, if the index is not
    /// null. Returns `Some(())` if the record was successfully
    /// updated.
    fn set_record(
        &mut self,
        ix: EdgeListIx,
        handle: Handle,
        next: EdgeListIx,
    ) -> Option<()> {
        let h_ix = ix.as_vec_ix()?.handle_ix();
        let n_ix = ix.as_vec_ix()?.next_ix();

        self.record_vec.set(h_ix, handle.as_integer());
        self.record_vec.set(n_ix, next.as_vec_value());

        Some(())
    }

    /// Follow the linked list pointer in the given record to the next
    /// entry, if it exists.
    fn next(&self, record: EdgeRecord) -> Option<EdgeRecord> {
        self.get_record(record.1)
    }
}
