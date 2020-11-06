use crate::{handle::Handle, packed::*};

use std::num::NonZeroUsize;

use super::graph::WIDE_PAGE_WIDTH;

use super::{OneBasedIndex, RecordIndex};

use super::list;
use super::list::{PackedList, PackedListMut};

/// The index for an edge record. Valid indices are natural numbers
/// starting from 1, each denoting a *record*. An edge list index of
/// zero denotes a lack of an edge, or the empty edge list.
///
/// As zero is used to represent no edge/the empty edge list,
/// `Option<NonZeroUsize>` is a natural fit for representing this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeListIx(Option<NonZeroUsize>);

crate::impl_one_based_index!(EdgeListIx);

/// The index into the underlying packed vector that is used to
/// represent the edge lists.

/// Each edge list record takes up two elements, so an `EdgeVecIx` is
/// always even. They also start from zero, so there's an offset by one
/// compared to `EdgeListIx`, besides the record size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct EdgeVecIx(usize);

impl RecordIndex for EdgeVecIx {
    const RECORD_WIDTH: usize = 2;

    #[inline]
    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self> {
        ix.to_record_start(Self::RECORD_WIDTH).map(EdgeVecIx)
    }

    #[inline]
    fn to_one_based_ix<I: OneBasedIndex>(self) -> I {
        I::from_record_start(self.0, Self::RECORD_WIDTH)
    }

    #[inline]
    fn record_ix(self, offset: usize) -> usize {
        self.0 + offset
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

pub type EdgeRecord = (Handle, EdgeListIx);

impl PackedList for EdgeLists {
    type ListPtr = EdgeListIx;
    type ListRecord = EdgeRecord;

    #[inline]
    fn next_pointer(rec: &EdgeRecord) -> EdgeListIx {
        rec.1
    }

    #[inline]
    fn get_record(&self, ptr: EdgeListIx) -> Option<EdgeRecord> {
        let handle = self.get_handle(ptr)?;
        let next = self.get_next(ptr)?;
        Some((handle, next))
    }

    #[inline]
    fn next_record(&self, rec: &EdgeRecord) -> Option<EdgeRecord> {
        self.next(*rec)
    }
}

impl PackedListMut for EdgeLists {
    type ListLink = EdgeListIx;

    #[inline]
    fn get_record_link(record: &EdgeRecord) -> EdgeListIx {
        record.1
    }

    #[inline]
    fn link_next(link: EdgeListIx) -> EdgeListIx {
        link
    }

    #[inline]
    fn remove_at_pointer(&mut self, ptr: EdgeListIx) -> Option<EdgeListIx> {
        let h_ix = ptr.to_record_ix(2, 0)?;
        let n_ix = h_ix + 1;

        let next = self.record_vec.get_unpack(n_ix);
        self.record_vec.set(h_ix, 0);
        self.record_vec.set(n_ix, 0);

        Some(next)
    }

    #[inline]
    fn remove_next(&mut self, ptr: EdgeListIx) -> Option<()> {
        let record_next_vec_ix = ptr.to_record_ix(2, 1)?;
        let next_edge_ix = self.record_vec.get_unpack(record_next_vec_ix);

        let next = self.remove_at_pointer(next_edge_ix)?;
        self.record_vec.set_pack(record_next_vec_ix, next);

        Some(())
    }
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            record_vec: PagedIntVec::new(WIDE_PAGE_WIDTH),
            removed_records: Vec::new(),
        }
    }
}

impl EdgeLists {
    /// Returns the number of edge records, i.e. the total number of
    /// edges. Subtracts the number of removed records.
    #[inline]
    pub(super) fn len(&self) -> usize {
        let num_records = self.record_vec.len() / EdgeVecIx::RECORD_WIDTH;
        num_records - self.removed_records.len()
    }

    /// Get the handle for the record at the index, if the index is
    /// not null.
    #[inline]
    fn get_handle(&self, ix: EdgeListIx) -> Option<Handle> {
        let h_ix = ix.to_record_ix(2, 0)?;
        let handle = Handle::from_integer(self.record_vec.get(h_ix));
        Some(handle)
    }

    /// Get the pointer to the following record, for the record at the
    /// index, if the index is not null. Will return `Some` even if
    /// the pointer is null, but the contained `EdgeListIx` will
    /// instead be null.
    #[inline]
    fn get_next(&self, ix: EdgeListIx) -> Option<EdgeListIx> {
        let n_ix = ix.to_record_ix(2, 1)?;
        let next = self.record_vec.get_unpack(n_ix);
        Some(next)
    }

    /// Create a new record with the provided contents and return its
    /// `EdgeListIx`.
    pub(super) fn append_record(
        &mut self,
        handle: Handle,
        next: EdgeListIx,
    ) -> EdgeListIx {
        let rec_ix = EdgeListIx::from_record_start(self.record_vec.len(), 2);
        self.record_vec.append(handle.pack());
        self.record_vec.append(next.pack());
        rec_ix
    }

    /// Create a new *empty* record and return its `EdgeListIx`.
    #[allow(dead_code)]
    #[must_use]
    fn append_empty(&mut self) -> EdgeListIx {
        let rec_ix = EdgeListIx::from_record_start(self.record_vec.len(), 2);
        self.record_vec.append(0);
        self.record_vec.append(0);
        rec_ix
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
        let h_ix = ix.to_record_ix(2, 0)?;
        let n_ix = ix.to_record_ix(2, 1)?;

        self.record_vec.set_pack(h_ix, handle);
        self.record_vec.set_pack(n_ix, next);

        Some(())
    }

    fn set_next(&mut self, ix: EdgeListIx, next: EdgeListIx) -> Option<()> {
        let n_ix = ix.to_record_ix(2, 1)?;
        self.record_vec.set_pack(n_ix, next);
        Some(())
    }

    fn clear_record(&mut self, ix: EdgeListIx) -> Option<()> {
        let h_ix = ix.to_record_ix(2, 0)?;
        let n_ix = h_ix + 1;

        self.record_vec.set(h_ix, 0);
        self.record_vec.set(n_ix, 0);

        Some(())
    }

    /// Follow the linked list pointer in the given record to the next
    /// entry, if it exists.
    fn next(&self, record: EdgeRecord) -> Option<EdgeRecord> {
        self.get_record(record.1)
    }

    /// Return an iterator that walks through the edge list starting
    /// at the provided index.
    pub fn iter(&self, ix: EdgeListIx) -> list::Iter<'_, Self> {
        list::Iter::new(self, ix)
    }

    pub fn iter_mut(&mut self, ix: EdgeListIx) -> list::IterMut<'_, Self> {
        list::IterMut::new(self, ix)
    }

    /// Updates the first edge record in the provided edge list that
    /// fulfills the predicate `pred`, using the provided update
    /// function `f`.
    ///
    /// If no edge record fulfills the predicate, does nothing and
    /// return `false`. Returns `true` if a record was updated.
    pub(super) fn update_edge_record<P, F>(
        &mut self,
        start: EdgeListIx,
        pred: P,
        f: F,
    ) -> bool
    where
        P: Fn(EdgeListIx, EdgeRecord) -> bool,
        F: Fn(EdgeRecord) -> EdgeRecord,
    {
        let entry = self.iter(start).find(|&(ix, rec)| pred(ix, rec));
        if let Some((edge_ix, record)) = entry {
            let (handle, next) = f(record);
            self.set_record(edge_ix, handle, next);
            true
        } else {
            false
        }
    }

    /// In the linked list that starts at the provided index, find the
    /// first edge record that fulfills the provided predicate, and
    /// remove it if it exists. Returns the index of the new start of
    /// the edge list.
    ///
    /// Since the new start of the index is returned, the output of
    /// this method can be directly used to update the corresponding
    /// GraphRecord.
    #[allow(dead_code)]
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

            let prec_next_vec_ix = prec_ix.to_record_ix(2, 1)?;
            // Update the previous `next` pointer
            self.record_vec.set_pack(prec_next_vec_ix, curr_record.1);
            // Mark the record in question as removed
            self.removed_records.push(curr_ix);
            // The start of the edge list hasn't changed
            Some(start)
        }
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
        edges.set_record(e_2, hnd(2), EdgeListIx::null());

        // edge list two, starting with e_3
        //  /- hnd(4)
        // B - hnd(5)
        //  \- hnd(6)
        edges.set_record(e_3, hnd(4), e_4);
        edges.set_record(e_4, hnd(5), e_5);
        edges.set_record(e_5, hnd(6), EdgeListIx::null());

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
    fn remove_edge_list_record_iter_mut() {
        let hnd = |x: u64| Handle::pack(x, false);

        let edgevec = |es: &EdgeLists, ix: EdgeListIx| {
            es.iter(ix).map(|(_, (h, _))| h).collect::<Vec<_>>()
        };

        let mut edges = EdgeLists::default();

        let vec_edge_list = |edges: &EdgeLists, head: EdgeListIx| {
            edges
                .iter(head)
                .map(|(edge, (handle, next))| {
                    let edge = edge.to_vector_value();
                    let handle = handle.as_integer();
                    let next = next.to_vector_value();
                    (edge, handle, next)
                })
                .collect::<Vec<_>>()
        };

        let print_edge_list = |edges: &EdgeLists, head: EdgeListIx| {
            println!("| {:>4} | {:>6} | {:>4} |", "edge", "handle", "next");
            for (edge, (handle, next)) in edges.iter(head) {
                let edge = edge.to_vector_value();
                let handle = handle.as_integer();
                let next = next.to_vector_value();
                println!("| {:>4} | {:>6} | {:>4} |", edge, handle, next);
            }
            println!();
        };

        let handles =
            vec![1, 2, 3, 4, 5].into_iter().map(hnd).collect::<Vec<_>>();

        let mut last_edge = EdgeListIx::null();

        let mut edge_ixs = Vec::new();

        // A single edge list, all edges have the same source and
        // different targets
        for &h in handles.iter() {
            let edge = edges.append_record(h, last_edge);
            println!("{:?} -> {:?}", edge, last_edge);
            edge_ixs.push(edge);
            last_edge = edge;
        }

        println!("{:#?}", edge_ixs);

        let head = *edge_ixs.last().unwrap();
        let tail = *edge_ixs.first().unwrap();
        println!("head\t{:?}", head);
        println!("tail\t{:?}", tail);

        assert_eq!(head.to_vector_value(), 5);
        assert_eq!(tail.to_vector_value(), 1);

        println!("original edge list");
        // println!("{:?}", edges.iter(head).collect::<Vec<_>>());

        let orig_edge_vec = vec_edge_list(&edges, head);

        print_edge_list(&edges, head);

        // Remove the first edge with an even node ID
        let new_head =
            edges.iter_mut(head).remove_record_with(|ix, (h, next)| {
                let id = u64::from(h.id());
                u64::from(h.id()) % 2 == 0
            });
        println!();

        assert_eq!(Some(head), new_head);

        let new_edge_vec = vec_edge_list(&edges, head);

        assert_eq!(
            new_edge_vec,
            vec![(5, 10, 3), (3, 6, 2), (2, 4, 1), (1, 2, 0)]
        );
        println!("new head\t{:?}", new_head);

        println!("edge list with first even node removed");
        print_edge_list(&edges, head);

        // Remove the first edge with an even handle
        // let rem_1 = edges
        //     .remove_edge_record(e_1, |(h, _)| usize::from(h.id()) % 2 == 0);
        // let mod_edges = edgevec(&edges, e_1);
        // The start of the list is still the same
        // assert_eq!(rem_1, Some(e_1));
        // assert_eq!(vec![hnd(1), hnd(3), hnd(4), hnd(5)], mod_edges);

        // Remove handle 5
        // let rem_last = edges.remove_edge_record(e_1, |(h, _)| h == hnd(5));
        // let mod_edges = edgevec(&edges, e_1);

        /*
        // The start of the list is still the same
        assert_eq!(rem_last, Some(e_1));
        assert_eq!(vec![hnd(1), hnd(3), hnd(4)], mod_edges);
        */

        // Remove the first record
        // Remove the first edge with an even handle, again

        // let rem_1st = edges.remove_edge_record(e_1, |(h, _)| h == hnd(1));

        // e_1 is still in the edge list, but marked as removed;
        // the start of the list is the value in rem_1st, which is now equal to e_3
        // let mod_edges = edgevec(&edges, rem_1st.unwrap());

        /*
        // The start of the list has changed
        assert_eq!(rem_1st, Some(e_3));
        assert_eq!(vec![hnd(3), hnd(4)], mod_edges);
        */
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
        edges.set_record(e_5, hnd(5), EdgeListIx::null());

        let edgevec = |es: &EdgeLists, ix: EdgeListIx| {
            es.iter(ix).map(|(_, (h, _))| h).collect::<Vec<_>>()
        };

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
