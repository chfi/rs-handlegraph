use std::num::NonZeroUsize;

use fnv::FnvHashMap;

use crate::{handle::Handle, packed::*};

use super::{
    defragment::Defragment,
    graph::WIDE_PAGE_WIDTH,
    list::{self, PackedList, PackedListMut},
    OneBasedIndex, RecordIndex,
};

/// The index for an edge record. Valid indices are natural numbers
/// starting from 1, each denoting a *record*. An edge list index of
/// zero denotes a lack of an edge, or the empty edge list.
///
/// As zero is used to represent no edge/the empty edge list,
/// `Option<NonZeroUsize>` is a natural fit for representing this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EdgeListIx(Option<NonZeroUsize>);

crate::impl_one_based_index!(EdgeListIx);
crate::impl_space_usage_stack_newtype!(EdgeListIx);

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
    pub record_vec: PagedIntVec,
    pub removed_records: Vec<EdgeListIx>,
    pub removed_count: usize,
    pub(crate) reversing_self_edge_records: usize,
    pub(crate) removed_reversing_self_edge_records: usize,
}

crate::impl_space_usage!(EdgeLists, [record_vec, removed_records]);

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

        self.removed_records.push(ptr);
        self.removed_count += 1;

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
            removed_count: 0,
            reversing_self_edge_records: 0,
            removed_reversing_self_edge_records: 0,
        }
    }
}

impl EdgeLists {
    pub fn log_len(&self) {
        use log::info;

        let num_records = self.record_vec.len() / EdgeVecIx::RECORD_WIDTH;
        info!("num_records: {}", num_records);
        let del_records = self.removed_count;

        let num_rev_self = self.reversing_self_edge_records;
        let del_rev_self = self.removed_reversing_self_edge_records;

        info!("Edges - total   record count    {}", num_records);
        info!("Edges - deleted record count    {}", del_records);
        info!("Edges - total   rev. self edges {}", num_rev_self);
        info!("Edges - deleted rev. self edges {}", del_rev_self);
        info!(
            "Edges - removed_records.len()   {}",
            self.removed_records.len()
        );
    }

    pub fn empty_records(&self) -> Vec<EdgeListIx> {
        let mut res = Vec::new();

        let records = self.record_count();
        for i in 0..records {
            let ix = i * 2;
            let hi = ix;

            let handle = self.record_vec.get(hi);
            if handle == 0 {
                let edge_ix = EdgeListIx::from_zero_based(i);
                res.push(edge_ix);
            }
        }

        res
    }

    /// Returns the number of edge records, i.e. the total number of
    /// edges. Subtracts the number of removed records.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        let num_records = self.record_vec.len() / EdgeVecIx::RECORD_WIDTH;
        let num_edges = (num_records + self.reversing_self_edge_records) / 2;
        num_edges - (self.removed_count / 2)
    }

    #[inline]
    pub(crate) fn record_count(&self) -> usize {
        self.record_vec.len() / EdgeVecIx::RECORD_WIDTH
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
    #[inline]
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

    /// Follow the linked list pointer in the given record to the next
    /// entry, if it exists.
    #[inline]
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
    #[inline]
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

    pub fn print_diagnostics(&self) {
        println!("\n ~~ BEGIN EdgeLists diagnostics ~~ \n");

        println!(" ----- {:^20} -----", "record_vec");
        self.record_vec.print_diagnostics();
        println!();

        println!("\n ~~  END  EdgeLists diagnostics ~~ \n");
    }

    pub fn save_diagnostics(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;

        let mut file = File::create(path)?;
        self.record_vec.save_diagnostics(&mut file)?;

        Ok(())
    }
}

impl Defragment for EdgeLists {
    type Updates = FnvHashMap<EdgeListIx, EdgeListIx>;

    fn defragment(&mut self) -> Option<Self::Updates> {
        let total_records = self.record_vec.len() / EdgeVecIx::RECORD_WIDTH;

        let mut new_record_vec = PagedIntVec::new(WIDE_PAGE_WIDTH);
        new_record_vec.reserve(self.len() * EdgeVecIx::RECORD_WIDTH);

        let mut id_map_: FnvHashMap<EdgeListIx, EdgeListIx> =
            FnvHashMap::default();

        let mut next_ix = 0usize;
        for ix in 0..total_records {
            let old_ix = EdgeListIx::from_zero_based(ix);
            let old_vec_ix = old_ix.to_record_ix(2, 0)?;
            let handle = self.record_vec.get(old_vec_ix);
            if handle != 0 {
                if !id_map_.contains_key(&old_ix) {
                    let new_ix = EdgeListIx::from_zero_based(next_ix);
                    next_ix += 1;
                    id_map_.insert(old_ix, new_ix);
                }
            } else {
                if !id_map_.contains_key(&old_ix) {
                    id_map_.insert(old_ix, EdgeListIx::null());
                } else {
                    log::info!("tried to replace edge index?");
                }
            }
        }

        for ix in 0..total_records {
            let old_ix = EdgeListIx::from_zero_based(ix);
            let old_vec_ix = old_ix.to_record_ix(2, 0)?;

            let handle = self.record_vec.get(old_vec_ix);

            if let Some(new_ix) = id_map_.get(&old_ix) {
                if handle != 0 && !new_ix.is_null() {
                    let old_next: EdgeListIx =
                        self.record_vec.get_unpack(old_vec_ix + 1);

                    if let Some(new_next) = id_map_.get(&old_next) {
                        new_record_vec.append(handle);
                        new_record_vec.append(new_next.pack());
                    } else {
                        new_record_vec.append(handle);
                        new_record_vec.append(0);
                    }
                    // if new_ix.is_null() {
                    //     panic!("this shouldn't happen!");
                    // }
                }
            }
        }

        self.record_vec = new_record_vec;
        self.removed_records.clear();
        self.removed_count = 0;
        self.reversing_self_edge_records -=
            self.removed_reversing_self_edge_records;
        self.removed_reversing_self_edge_records = 0;

        Some(id_map_)
    }

    /*
    /// Defragments the edge list record vector and return a map
    /// describing how the indices of the still-existing records are
    /// transformed. Uses the `removed_records` vector, and empties it.
    ///
    /// Returns None if there are no removed records.
    fn defragment(&mut self) -> Option<Self::Updates> {
        let total_records = self.record_vec.len() / EdgeVecIx::RECORD_WIDTH;
        let id_map = defragment::build_id_map_1_based(
            &mut self.removed_records,
            total_records,
        )?;

        let mut new_record_vec = PagedIntVec::new(WIDE_PAGE_WIDTH);
        new_record_vec.reserve(self.len() * EdgeVecIx::RECORD_WIDTH);

        (0..total_records)
            .filter_map(|ix| {
                let old_ix = EdgeListIx::from_zero_based(ix);
                let old_vec_ix = old_ix.to_record_ix(2, 0)?;

                let _new_ix = id_map.get(&old_ix)?;
                let handle = self.record_vec.get(old_vec_ix);
                let next = self.record_vec.get_unpack(old_vec_ix + 1);
                let next = id_map.get(&next).copied().unwrap_or(next);

                Some((handle, next))
            })
            .for_each(|(handle, next)| {
                new_record_vec.append(handle);
                new_record_vec.append(next.pack());
            });

        self.record_vec = new_record_vec;
        self.removed_records.clear();
        self.removed_count = 0;
        self.reversing_self_edge_records -=
            self.removed_reversing_self_edge_records;
        self.removed_reversing_self_edge_records = 0;

        Some(id_map)
    }
    */
}

#[cfg(test)]
pub(crate) mod tests {
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

    pub(crate) fn vec_edge_list(
        edges: &EdgeLists,
        head: EdgeListIx,
    ) -> Vec<(u64, u64, u64)> {
        edges
            .iter(head)
            .map(|(edge, (handle, next))| {
                let edge = edge.to_vector_value();
                let handle = handle.as_integer();
                let next = next.to_vector_value();
                (edge, handle, next)
            })
            .collect::<Vec<_>>()
    }

    #[allow(dead_code)]
    pub(crate) fn vec_edge_list_records(
        edges: &EdgeLists,
    ) -> Vec<(u64, u64, u64)> {
        let mut results = Vec::new();

        for ix in 0..edges.record_count() {
            let edge_ix = EdgeListIx::from_zero_based(ix);
            let (handle, ptr) = edges.get_record(edge_ix).unwrap();
            results.push((edge_ix.pack(), u64::from(handle.id()), ptr.pack()));
        }

        results
    }

    #[test]
    fn remove_edge_list_record_iter_mut() {
        let hnd = |x: u64| Handle::pack(x, false);

        let mut edges = EdgeLists::default();

        let handles =
            vec![1, 2, 3, 4, 5].into_iter().map(hnd).collect::<Vec<_>>();

        let mut last_edge = EdgeListIx::null();

        let mut edge_ixs = Vec::new();

        // A single edge list, all edges have the same source and
        // different targets
        for &h in handles.iter() {
            let edge = edges.append_record(h, last_edge);
            edge_ixs.push(edge);
            last_edge = edge;
        }

        let head = *edge_ixs.last().unwrap();
        let tail = *edge_ixs.first().unwrap();

        assert_eq!(head.to_vector_value(), 5);
        assert_eq!(tail.to_vector_value(), 1);

        // Remove the first edge with an even node ID
        let new_head = edges
            .iter_mut(head)
            .remove_record_with(|_ix, (h, _next)| u64::from(h.id()) % 2 == 0);

        assert_eq!(Some(head), new_head);
        let new_edge_vec = vec_edge_list(&edges, head);

        assert_eq!(
            new_edge_vec,
            vec![(5, 10, 3), (3, 6, 2), (2, 4, 1), (1, 2, 0)]
        );

        // Remove the last record of the list
        let new_head = edges
            .iter_mut(head)
            .remove_record_with(|_ix, (_h, next)| next.is_null());

        assert_eq!(Some(head), new_head);

        let new_edge_vec = vec_edge_list(&edges, head);
        assert_eq!(new_edge_vec, vec![(5, 10, 3), (3, 6, 2), (2, 4, 0)]);

        // Remove the head of the list
        let new_head =
            edges.iter_mut(head).remove_record_with(|ix, _| ix == head);

        let new_edge_vec = vec_edge_list(&edges, head);
        assert_eq!(new_edge_vec, vec![(5, 0, 0)]);

        let new_edge_vec = vec_edge_list(&edges, new_head.unwrap());
        assert_eq!(new_edge_vec, vec![(3, 6, 2), (2, 4, 0)]);
        assert_eq!(new_head.unwrap().pack(), 3);

        // Remove the rest of the edges one at a time
        let new_head = edges
            .iter_mut(new_head.unwrap())
            .remove_record_with(|_, _| true);

        let new_edge_vec = vec_edge_list(&edges, new_head.unwrap());
        assert_eq!(new_edge_vec, vec![(2, 4, 0)]);
        assert_eq!(new_head.unwrap().pack(), 2);

        let new_head = edges
            .iter_mut(new_head.unwrap())
            .remove_record_with(|_, _| true);

        let new_edge_vec = vec_edge_list(&edges, new_head.unwrap());
        assert!(new_edge_vec.is_empty());
        assert_eq!(new_head.unwrap().pack(), 0);

        let new_head = edges
            .iter_mut(new_head.unwrap())
            .remove_record_with(|_, _| true);
        assert_eq!(new_head, None);
    }

    #[test]
    fn remove_many_edge_records() {
        let hnd = |x: u64| Handle::pack(x, false);

        let mut edges = EdgeLists::default();

        let handles =
            vec![1, 2, 3, 4, 5].into_iter().map(hnd).collect::<Vec<_>>();

        let mut last_edge = EdgeListIx::null();

        let mut edge_ixs = Vec::new();

        // A single edge list, all edges have the same source and
        // different targets
        for &h in handles.iter() {
            let edge = edges.append_record(h, last_edge);
            edge_ixs.push(edge);
            last_edge = edge;
        }

        let head = *edge_ixs.last().unwrap();
        let tail = *edge_ixs.first().unwrap();

        assert_eq!(head.to_vector_value(), 5);
        assert_eq!(tail.to_vector_value(), 1);

        // Remove all odd nodes
        let new_head = edges
            .iter_mut(head)
            .remove_all_records_with(|_, (h, _)| u64::from(h.id()) % 2 == 1);

        assert_eq!(new_head.unwrap().to_vector_value(), 4);
        let new_edge_vec = vec_edge_list(&edges, new_head.unwrap());
        assert!(new_edge_vec.iter().all(|&(_, h, _)| h % 2 == 0));

        // Remove all even nodes
        let new_head = edges
            .iter_mut(head)
            .remove_all_records_with(|_, (h, _)| u64::from(h.id()) % 2 == 0);
        assert_eq!(new_head, Some(EdgeListIx::null()));
        let new_edge_vec = vec_edge_list(&edges, new_head.unwrap());
        assert!(new_edge_vec.is_empty());
    }

    #[test]
    fn edges_defrag() {
        let mut edges = EdgeLists::default();

        let hnd = |x: u64| Handle::pack(x, false);
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();

        let append_slice = |edges: &mut EdgeLists, handles: &[Handle]| {
            let mut last = EdgeListIx::null();
            let mut edge_ixs = Vec::new();
            for &h in handles.iter() {
                let edge = edges.append_record(h, last);
                edge_ixs.push(edge);
                last = edge;
            }
            edge_ixs
        };

        let edges_vec = |edges: &EdgeLists, head: EdgeListIx| {
            edges
                .iter(head)
                .map(|(_, (h, x))| {
                    let h = u64::from(h.id());
                    let x = x.pack();
                    (h, x)
                })
                .collect::<Vec<_>>()
        };

        let _list_1 =
            append_slice(&mut edges, &vec_hnd(vec![100, 101, 102, 103]));
        let _list_2 =
            append_slice(&mut edges, &vec_hnd(vec![200, 201, 202, 203]));
        let _list_3 =
            append_slice(&mut edges, &vec_hnd(vec![300, 301, 302, 303]));

        let edge_ix = |x: usize| EdgeListIx::from_one_based(x);

        let head_1 = edge_ix(4);
        let head_2 = edge_ix(8);
        let head_3 = edge_ix(12);

        let remove_edge_in =
            |edges: &mut EdgeLists, head: EdgeListIx, rem: usize| {
                let rem_ix = EdgeListIx::from_one_based(rem);
                edges
                    .iter_mut(head)
                    .remove_record_with(|x, _| x == rem_ix)
                    .unwrap()
            };

        // Remove edges at indices 4, 6, 7, 11
        let new_head_1 = remove_edge_in(&mut edges, head_1, 4);

        let new_head_2 = remove_edge_in(&mut edges, head_2, 7);
        let new_head_2 = remove_edge_in(&mut edges, new_head_2, 6);

        let new_head_3 = remove_edge_in(&mut edges, head_3, 11);

        let defrag_map_1 = edges.defragment().unwrap();

        let new_head_1 = *defrag_map_1.get(&new_head_1).unwrap();
        let new_head_2 = *defrag_map_1.get(&new_head_2).unwrap();
        let new_head_3 = *defrag_map_1.get(&new_head_3).unwrap();

        assert_eq!(
            edges_vec(&edges, new_head_1),
            vec![(102, 2), (101, 1), (100, 0)]
        );

        assert_eq!(edges_vec(&edges, new_head_2), vec![(203, 4), (200, 0)]);

        assert_eq!(
            edges_vec(&edges, new_head_3),
            vec![(303, 7), (301, 6), (300, 0)]
        );

        let new_head_1 = remove_edge_in(&mut edges, new_head_1, 2);
        let new_head_1 = remove_edge_in(&mut edges, new_head_1, 1);

        let new_head_3 = remove_edge_in(&mut edges, new_head_3, 7);
        let new_head_3 = remove_edge_in(&mut edges, new_head_3, 8);

        let defrag_map_2 = edges.defragment().unwrap();

        let new_head_1 = *defrag_map_2.get(&new_head_1).unwrap();
        let new_head_2 = *defrag_map_2.get(&new_head_2).unwrap();
        let new_head_3 = *defrag_map_2.get(&new_head_3).unwrap();

        assert_eq!(edges_vec(&edges, new_head_1), vec![(102, 0)]);
        assert_eq!(edges_vec(&edges, new_head_2), vec![(203, 2), (200, 0)]);
        assert_eq!(edges_vec(&edges, new_head_3), vec![(300, 0)]);
    }
}
