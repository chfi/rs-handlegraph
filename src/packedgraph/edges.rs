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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    /// Returns the "null", or empty `EdgeListIx`, i.e. the one that
    /// represents the empty list when used as a pointer in an edge
    /// list.
    pub(super) fn empty() -> Self {
        Self(None)
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
    pub(super) fn as_vec_ix(&self) -> Option<EdgeVecIx> {
        let x = self.0?.get();
        Some(EdgeVecIx((x - 1) * 2))
    }
}

/// The index into the underlying packed vector that is used to
/// represent the edge lists.

/// Each edge list record takes up two elements, so an `EdgeVecIx` is
/// always even. They also start from zero, so there's an offset by one
/// compared to `EdgeListIx`, besides the record size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub(super) fn as_list_ix(&self) -> EdgeListIx {
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
    removed_records: Vec<EdgeListIx>,
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            record_vec: PagedIntVec::new(WIDE_PAGE_WIDTH),
            removed_records: Vec::new(),
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

    /// Get the handle for the record at the index, if the index is
    /// not null.
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
    pub(super) fn get_record(&self, ix: EdgeListIx) -> Option<EdgeRecord> {
        let handle = self.get_handle(ix)?;
        let next = self.get_next(ix)?;
        Some((handle, next))
    }

    /// Create a new *empty* record and return its `EdgeListIx`.
    #[must_use]
    pub(super) fn append_empty(&mut self) -> EdgeListIx {
        let vec_ix = EdgeVecIx::new(self.record_vec.len());
        self.record_vec.append(0);
        self.record_vec.append(0);
        vec_ix.as_list_ix()
    }

    /// Create a new record with the provided contents and return its
    /// `EdgeListIx`.
    pub(super) fn append_record(
        &mut self,
        handle: Handle,
        next: EdgeListIx,
    ) -> EdgeListIx {
        let vec_ix = EdgeVecIx::new(self.record_vec.len());
        self.record_vec.append(handle.as_integer());
        self.record_vec.append(next.as_vec_value());
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

    /// Return an iterator that walks through the edge list starting
    /// at the provided index.
    pub fn iter(&self, ix: EdgeListIx) -> EdgeListIter<'_> {
        EdgeListIter::new(self, ix)
    }

    /// In the linked list that starts at the provided index, find the
    /// first edge record that fulfills the provided predicate, and
    /// remove it if it exists. Returns the index of the new start of
    /// the edge list.
    ///
    /// Since the new start of the index is returned, the output of
    /// this method can be directly used to update the corresponding
    /// GraphRecord.
    #[must_use]
    pub(super) fn remove_edge_record<P>(
        &mut self,
        start: EdgeListIx,
        pred: P,
    ) -> Option<EdgeListIx>
    where
        P: Fn(EdgeRecord) -> bool,
    {
        let list_step = self.iter(start).position(|(_, rec)| pred(rec))?;

        if list_step == 0 {
            // If the edge record to remove is the very first, the new
            // start of the list is the second record.
            let next = self.get_next(start)?;
            self.removed_records.push(start);
            Some(next)
        } else {
            // If the edge record is at position I for I in [1..N],
            // the start of the list is the same, but we need to
            // change the `next` pointer of the preceding record in
            // the list, to that of the record to remove.

            let (prec_ix, _prec_record) =
                self.iter(start).nth(list_step - 1)?;
            let (curr_ix, curr_record) = self.iter(start).nth(list_step)?;

            let prec_next_vec_ix = prec_ix.as_vec_ix()?.next_ix();
            // Update the previous `next` pointer
            self.record_vec
                .set(prec_next_vec_ix, curr_record.1.as_vec_value());
            // Mark the record in question as removed
            self.removed_records.push(curr_ix);
            // The start of the edge list hasn't changed
            Some(start)
        }
    }
}

/// An iterator through a linked list of edge records. Yields the
/// current `EdgeListIx`, as well as its record, until the end of the
/// list has been reached.
pub struct EdgeListIter<'a> {
    edge_lists: &'a EdgeLists,
    current_index: EdgeListIx,
    current_record: Option<EdgeRecord>,
}

impl<'a> EdgeListIter<'a> {
    fn new(edge_lists: &'a EdgeLists, start: EdgeListIx) -> Self {
        let current_record = edge_lists.get_record(start);
        let current_index = start;
        Self {
            edge_lists,
            current_index,
            current_record,
        }
    }
}

impl<'a> Iterator for EdgeListIter<'a> {
    // (EdgeListIx, (Handle, EdgeListIx));
    type Item = (EdgeListIx, EdgeRecord);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let record = self.current_record?;
        let next_record = self.edge_lists.next(record);
        let this_ix = self.current_index;
        let (handle, next) = record;
        self.current_record = next_record;
        self.current_index = next;
        Some((this_ix, (handle, next)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_edges_iter() {
        let mut edges = EdgeLists::default();

        let hnd = |x: u64| Handle::pack(x, false);

        let e_1 = edges.append_empty();
        let e_2 = edges.append_empty();

        let e_3 = edges.append_empty();
        let e_4 = edges.append_empty();
        let e_5 = edges.append_empty();

        // edge list one, starting with e_1
        //  /- hnd(1)
        // A
        //  \- hnd(2)
        edges.set_record(e_1, hnd(1), e_2);
        edges.set_record(e_2, hnd(2), EdgeListIx::empty());

        // edge list two, starting with e_3
        //  /- hnd(4)
        // B - hnd(5)
        //  \- hnd(6)
        edges.set_record(e_3, hnd(4), e_4);
        edges.set_record(e_4, hnd(5), e_5);
        edges.set_record(e_5, hnd(6), EdgeListIx::empty());

        let l_1 = edges.iter(e_1).map(|(_, (h, _))| h).collect::<Vec<_>>();
        let l_2 = edges.iter(e_2).map(|(_, (h, _))| h).collect::<Vec<_>>();
        assert_eq!(vec![hnd(1), hnd(2)], l_1);
        assert_eq!(vec![hnd(2)], l_2);

        let l_3 = edges.iter(e_3).map(|(_, (h, _))| h).collect::<Vec<_>>();
        let l_4 = edges.iter(e_4).map(|(_, (h, _))| h).collect::<Vec<_>>();
        let l_5 = edges.iter(e_5).map(|(_, (h, _))| h).collect::<Vec<_>>();
        assert_eq!(vec![hnd(4), hnd(5), hnd(6)], l_3);
        assert_eq!(vec![hnd(5), hnd(6)], l_4);
        assert_eq!(vec![hnd(6)], l_5);
    }

    #[test]
    fn packedgraph_edges_remove_record() {
        let mut edges = EdgeLists::default();

        let hnd = |x: u64| Handle::pack(x, false);

        let e_1 = edges.append_empty();
        let e_2 = edges.append_empty();
        let e_3 = edges.append_empty();
        let e_4 = edges.append_empty();
        let e_5 = edges.append_empty();

        // A single edge list, all edges have the same source and
        // different targets

        edges.set_record(e_1, hnd(1), e_2);
        edges.set_record(e_2, hnd(2), e_3);
        edges.set_record(e_3, hnd(3), e_4);
        edges.set_record(e_4, hnd(4), e_5);
        edges.set_record(e_5, hnd(5), EdgeListIx::empty());

        let edgevec = |es: &EdgeLists, ix: EdgeListIx| {
            es.iter(ix).map(|(_, (h, _))| h).collect::<Vec<_>>()
        };

        let orig_edges = edgevec(&edges, e_1);

        // Remove the first edge with an even handle
        let rem_1 = edges
            .remove_edge_record(e_1, |(h, _)| usize::from(h.id()) % 2 == 0);
        let mod_edges = edgevec(&edges, e_1);
        // The start of the list is still the same
        assert_eq!(rem_1, Some(e_1));
        assert_eq!(vec![hnd(1), hnd(3), hnd(4), hnd(5)], mod_edges);

        // Remove handle 5
        let rem_last = edges.remove_edge_record(e_1, |(h, _)| h == hnd(5));
        let mod_edges = edgevec(&edges, e_1);
        // The start of the list is still the same
        assert_eq!(rem_last, Some(e_1));
        assert_eq!(vec![hnd(1), hnd(3), hnd(4)], mod_edges);

        // Remove the first record
        // Remove the first edge with an even handle, again
        let rem_1st = edges.remove_edge_record(e_1, |(h, _)| h == hnd(1));
        // e_1 is still in the edge list, but marked as removed;
        // the start of the list is the value in rem_1st, which is now equal to e_3
        let mod_edges = edgevec(&edges, rem_1st.unwrap());
        // The start of the list has changed
        assert_eq!(rem_1st, Some(e_3));
        assert_eq!(vec![hnd(3), hnd(4)], mod_edges);
    }
}
