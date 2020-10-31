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

pub(crate) static NARROW_PAGE_WIDTH: usize = 256;
pub(crate) static WIDE_PAGE_WIDTH: usize = 1024;

pub use super::edges::{EdgeListIter, EdgeListIx, EdgeLists, EdgeVecIx};
pub use super::sequence::{PackedSeqIter, Sequences};

/// The index for a graph record. Valid indices are natural numbers
/// above zero, each denoting a 2-element record. An index of zero
/// denotes a record that doesn't exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphRecordIx(Option<NonZeroUsize>);

impl GraphRecordIx {
    /// Create a new `GraphRecordIx` by wrapping a `usize`. Should only
    /// be used in the PackedGraph graph record internals.
    ///
    /// If `x` is zero, the result will be `GraphRecordIx(None)`.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    /// Returns the "null", or empty `GraphRecordIx`, i.e. the one that
    /// is used for yet-to-be-filled elements in the graph NodeId map.
    #[inline]
    pub(super) fn empty() -> Self {
        Self(None)
    }

    /// Returns `true` if the provided `GraphRecordIx` represents the
    /// null record.
    #[inline]
    pub(super) fn is_null(&self) -> bool {
        self.0.is_none()
    }

    /// Unwrap the `GraphRecordIx` into a `u64` for use as a value in
    /// a packed vector.
    #[inline]
    pub(super) fn as_vec_value(&self) -> u64 {
        match self.0 {
            None => 0,
            Some(v) => v.get() as u64,
        }
    }

    /// Wrap a `u64`, e.g. an element from a packed vector, as a
    /// `GraphRecordIx`.
    #[inline]
    pub(super) fn from_vec_value(x: u64) -> Self {
        Self(NonZeroUsize::new(x as usize))
    }

    /// Transforms the `GraphRecordIx` into an index that can be used
    /// to get the first element of a record from the graph record
    /// vector. Returns None if the `GraphRecordIx` points to the
    /// empty record.
    ///
    /// `x -> (x - 1) * 2`
    #[inline]
    pub(super) fn as_vec_ix(&self) -> Option<GraphVecIx> {
        let x = self.0?.get();
        Some(GraphVecIx((x - 1) * 2))
    }
}

/// The index into the underlying packed vector that is used to
/// represent the graph records that hold pointers to the two edge
/// lists for each node.

/// Each graph record takes up two elements, so a `GraphVecIx` is
/// always even.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct GraphVecIx(usize);

impl GraphVecIx {
    /// Create a new `GraphVecIx` by wrapping a `usize`. Should only be
    /// used in the PackedGraph graph record internals.
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }

    /// Transforms the `GraphVecIx` into an index that denotes a graph
    /// record. The resulting `GraphRecordIx` will always contain a
    /// value, never `None`.
    ///
    /// `x -> (x / 2) + 1`
    #[inline]
    pub(super) fn as_record_ix(&self) -> GraphRecordIx {
        GraphRecordIx::new((self.0 / 2) + 1)
    }

    #[inline]
    pub(super) fn left_edges_ix(&self) -> usize {
        self.0
    }

    #[inline]
    pub(super) fn right_edges_ix(&self) -> usize {
        self.0 + 1
    }
}

#[derive(Debug, Clone)]
pub struct NodeIdIndexMap {
    deque: PackedDeque,
    max_id: u64,
    min_id: u64,
}

impl Default for NodeIdIndexMap {
    fn default() -> Self {
        Self {
            deque: Default::default(),
            max_id: 0,
            min_id: std::u64::MAX,
        }
    }
}

impl NodeIdIndexMap {
    fn new() -> Self {
        Default::default()
    }

    /// Appends the provided NodeId to the Node id -> Graph index map,
    /// with the given target `GraphRecordIx`.
    ///
    /// Returns `true` if the NodeId was successfully appended.
    fn append_node_id(&mut self, id: NodeId, next_ix: GraphRecordIx) -> bool {
        let id = u64::from(id);
        if id == 0 {
            return false;
        }

        if self.deque.is_empty() {
            self.deque.push_back(0);
        } else {
            if id < self.min_id {
                let to_prepend = self.min_id - id;
                for _ in 0..to_prepend {
                    self.deque.push_front(0);
                }
            }

            if id > self.max_id {
                let ix = (id - self.min_id) as usize;
                if let Some(to_append) = ix.checked_sub(self.deque.len()) {
                    for _ in 0..=to_append {
                        self.deque.push_back(0);
                    }
                }
            }
        }

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = next_ix;

        self.deque.set(index as usize, value.as_vec_value());

        true
    }

    fn has_node<I: Into<NodeId>>(&self, id: I) -> bool {
        let id = u64::from(id.into());
        if id < self.min_id || id > self.max_id {
            return false;
        }
        let index = u64::from(id) - self.min_id;
        let value = self.deque.get(index as usize);
        let rec_ix = GraphRecordIx::from_vec_value(value);
        rec_ix.is_null()
    }
}

#[derive(Debug, Clone)]
pub struct NodeRecords {
    records_vec: PagedIntVec,
    id_index_map: NodeIdIndexMap,
    // sequences: Sequences,
}

impl Default for NodeRecords {
    fn default() -> NodeRecords {
        Self {
            records_vec: PagedIntVec::new(NARROW_PAGE_WIDTH),
            id_index_map: Default::default(),
        }
    }
}

impl NodeRecords {
    pub fn min_id(&self) -> u64 {
        self.id_index_map.min_id
    }

    pub fn max_id(&self) -> u64 {
        self.id_index_map.max_id
    }

    /// Return the `GraphRecordIx` that will be used by the next node
    /// that's inserted into the graph.
    fn next_graph_ix(&self) -> GraphRecordIx {
        let rec_count = self.records_vec.len();
        let vec_ix = GraphVecIx::new(rec_count);
        vec_ix.as_record_ix()
    }

    /// Append a new node graph record, using the provided
    /// `GraphRecordIx` no ensure that the record index is correctly
    /// synced.
    #[must_use]
    fn append_node_graph_record(
        &mut self,
        g_rec_ix: GraphRecordIx,
    ) -> Option<GraphRecordIx> {
        if self.next_graph_ix() != g_rec_ix {
            return None;
        }
        self.records_vec.append(0);
        self.records_vec.append(0);
        Some(g_rec_ix)
    }

    pub(super) fn insert_node(
        &mut self,
        n_id: NodeId,
    ) -> Option<GraphRecordIx> {
        let id = u64::from(n_id);
        if id == 0 {
            return None;
        }

        let next_ix = self.next_graph_ix();

        // TODO also add the sequence records

        if !self.id_index_map.append_node_id(n_id, next_ix) {
            return None;
        }
        let record_ix = self.append_node_graph_record(next_ix)?;
        Some(record_ix)
    }

    #[inline]
    pub(super) fn get_left_edge_list(
        &self,
        g_ix: GraphRecordIx,
    ) -> Option<EdgeListIx> {
        let vec_ix = g_ix.as_vec_ix()?;
        let left = vec_ix.left_edges_ix();
        let left = EdgeListIx::from_vec_value(self.records_vec.get(left));
        Some(left)
    }

    #[inline]
    pub(super) fn get_right_edge_list(
        &self,
        g_ix: GraphRecordIx,
    ) -> Option<EdgeListIx> {
        let vec_ix = g_ix.as_vec_ix()?;
        let right = vec_ix.right_edges_ix();
        let right = EdgeListIx::from_vec_value(self.records_vec.get(right));
        Some(right)
    }

    #[inline]
    pub(super) fn get_graph_record(
        &self,
        g_ix: GraphRecordIx,
    ) -> Option<(EdgeListIx, EdgeListIx)> {
        let vec_ix = g_ix.as_vec_ix()?;

        let left = vec_ix.left_edges_ix();
        let left = EdgeListIx::from_vec_value(self.records_vec.get(left));

        let right = vec_ix.right_edges_ix();
        let right = EdgeListIx::from_vec_value(self.records_vec.get(right));

        Some((left, right))
    }

    pub(super) fn set_node_edge_lists(
        &mut self,
        g_ix: GraphRecordIx,
        left: EdgeListIx,
        right: EdgeListIx,
    ) -> Option<()> {
        let vec_ix = g_ix.as_vec_ix()?;
        let left_ix = vec_ix.left_edges_ix();
        let right_ix = vec_ix.right_edges_ix();
        self.records_vec.set(left_ix, left.as_vec_value());
        self.records_vec.set(right_ix, right.as_vec_value());
        Some(())
    }

    #[inline]
    pub(super) fn update_node_edge_lists<F>(
        &mut self,
        g_ix: GraphRecordIx,
        f: F,
    ) -> Option<()>
    where
        F: Fn(EdgeListIx, EdgeListIx) -> (EdgeListIx, EdgeListIx),
    {
        let vec_ix = g_ix.as_vec_ix()?;
        let (left_rec, right_rec) = self.get_graph_record(g_ix)?;

        let (new_left, new_right) = f(left_rec, right_rec);

        let left = vec_ix.left_edges_ix();
        let right = vec_ix.right_edges_ix();
        self.records_vec.set(left, new_left.as_vec_value());
        self.records_vec.set(right, new_right.as_vec_value());
        Some(())
    }
}

#[derive(Debug, Clone)]
pub struct GraphRecord {
    left_edges: EdgeListIx,
    right_edges: EdgeListIx,
}

impl GraphRecord {
    pub(super) const SIZE: usize = 2;
    pub(super) const START_OFFSET: usize = 0;
    pub(super) const END_OFFSET: usize = 1;
}

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub(super) graph_records: PagedIntVec,
    pub(super) sequences: Sequences,
    pub(super) edges: EdgeLists,
    pub(super) id_graph_map: PackedDeque,
    pub(super) max_id: u64,
    pub(super) min_id: u64,
}

impl Default for PackedGraph {
    fn default() -> Self {
        let sequences = Default::default();
        let edges = Default::default();
        let id_graph_map = Default::default();
        let graph_records = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let max_id = 0;
        let min_id = std::u64::MAX;
        PackedGraph {
            sequences,
            edges,
            graph_records,
            id_graph_map,
            max_id,
            min_id,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct GraphIx(usize);

impl GraphIx {
    pub(super) fn to_id_map_entry(&self) -> u64 {
        let ix = self.0 as u64;
        ix + 1
    }

    pub(super) fn from_id_map_entry(ix: u64) -> Option<Self> {
        if ix == 0 {
            None
        } else {
            Some(GraphIx((ix - 1) as usize))
        }
    }

    pub(super) fn from_graph_records_ix(ix: usize) -> Self {
        GraphIx(ix / GraphRecord::SIZE)
    }

    pub(super) fn to_seq_record_ix(&self) -> usize {
        let ix = self.0;
        ix * Sequences::SIZE
    }

    pub(super) fn left_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::START_OFFSET
    }

    pub(super) fn right_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::END_OFFSET
    }
}

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub(super) fn new_record_ix(&mut self) -> GraphIx {
        let new_ix = self.graph_records.len();
        self.graph_records.append(0);
        self.graph_records.append(0);
        self.sequences.lengths.append(0);
        self.sequences.indices.append(0);
        GraphIx::from_graph_records_ix(new_ix)
    }

    pub(super) fn get_graph_record(&self, ix: GraphIx) -> GraphRecord {
        unimplemented!();
        // let left_edges = self.graph_records.get(ix.left_edges_ix()) as usize;
        // let right_edges = self.graph_records.get(ix.right_edges_ix()) as usize;
        // GraphRecord {
        //     left_edges,
        //     right_edges,
        // }
    }

    /*
    pub(super) fn graph_ix_left_edges(&self, ix: GraphIx) -> EdgeListIx {
        let left_edges_start =
            self.graph_records.get(ix.left_edges_ix()) as usize;
        EdgeListIx::new(left_edges_start)
    }

    pub(super) fn graph_ix_right_edges(&self, ix: GraphIx) -> EdgeListIx {
        let left_edges_start =
            self.graph_records.get(ix.left_edges_ix()) as usize;
        EdgeListIx::new(left_edges_start)
    }
    */

    pub(super) fn get_node_index(&self, id: NodeId) -> Option<GraphIx> {
        let id = u64::from(id);
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let map_ix = id - self.min_id;
        let ix = self.id_graph_map.get(map_ix as usize);
        GraphIx::from_id_map_entry(ix)
    }

    pub(super) fn handle_graph_ix(&self, handle: Handle) -> Option<GraphIx> {
        self.get_node_index(handle.id())
    }

    pub(super) fn push_node_record(&mut self, id: NodeId) -> GraphIx {
        let next_ix = self.new_record_ix();

        if self.id_graph_map.is_empty() {
            self.id_graph_map.push_back(0);
        } else {
            let id = u64::from(id);
            if id < self.min_id {
                let to_prepend = self.min_id - id;
                for _ in 0..to_prepend {
                    self.id_graph_map.push_front(0);
                }
            }
            if id > self.max_id {
                let ix = (id - self.min_id) as usize;
                if let Some(to_append) = ix.checked_sub(self.id_graph_map.len())
                {
                    for _ in 0..=to_append {
                        self.id_graph_map.push_back(0);
                    }
                }
            }
        }

        let id = u64::from(id);

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = next_ix.to_id_map_entry();

        self.id_graph_map.set(index as usize, value);

        next_ix
    }

    /*
    #[inline]
    pub(super) fn append_graph_record_start_edge(
        &mut self,
        g_ix: GraphIx,
        edge: EdgeIx,
    ) {
        self.graph_records.set(g_ix.left_edges_ix(), edge.0 as u64);
    }

    #[inline]
    pub(super) fn append_graph_record_end_edge(
        &mut self,
        g_ix: GraphIx,
        edge: EdgeIx,
    ) {
        self.graph_records.set(g_ix.right_edges_ix(), edge.0 as u64);
    }

    */

    #[inline]
    pub(super) fn swap_graph_record_elements(&mut self, a: usize, b: usize) {
        let a_val = self.graph_records.get(a);
        let b_val = self.graph_records.get(b);
        self.graph_records.set(a, b_val);
        self.graph_records.set(b, a_val);
    }

    #[inline]
    pub(super) fn handle_edge_record_ix(
        &self,
        handle: Handle,
        dir: Direction,
    ) -> usize {
        let g_ix = self.handle_graph_ix(handle).unwrap();

        use Direction as Dir;
        match (handle.is_reverse(), dir) {
            (false, Dir::Left) => g_ix.left_edges_ix(),
            (false, Dir::Right) => g_ix.right_edges_ix(),
            (true, Dir::Left) => g_ix.right_edges_ix(),
            (true, Dir::Right) => g_ix.left_edges_ix(),
        }
    }

    /*
    /// Get the EdgeList record index at the provided graph record
    /// *element* index. I.e. if `ix` is even, the first element of
    /// some record will be returned, if odd, the second element.
    #[inline]
    pub(super) fn get_edge_list_ix(&self, ix: usize) -> EdgeIx {
        let entry = self.graph_records.get(ix);
        EdgeIx(entry as usize)
    }
    */

    /*
    pub(super) fn remove_handle_from_edge(
        &mut self,
        handle: Handle,
        dir: Direction,
        to_remove: Handle,
    ) {
        let graph_rec_ix = self.handle_edge_record_ix(handle, dir);
        let edge_ix = self.get_edge_list_ix(graph_rec_ix);

        if let Some(ix) = self.edges.remove_handle_from_list(edge_ix, to_remove)
        {
            if ix != edge_ix {
                self.graph_records.set(graph_rec_ix, ix.0 as u64);
            }
        }
    }
    */

    pub(super) fn append_record_handle(&mut self) -> Handle {
        let id = NodeId::from(self.max_id + 1);

        let new_ix = self.graph_records.len();
        self.graph_records.append(0);
        self.graph_records.append(0);

        // let graph_ix = self.push_node_record(id);
        // let rec_ix = graph_ix.to_seq_record_ix();
        // self.sequences.set_record(rec_ix, seq_ix, length);
        Handle::pack(id, false)
        // graph_ix
    }

    /*
    pub(super) fn remove_handle(&mut self, handle: Handle) {
        let g_ix = self.handle_graph_ix(handle).unwrap();

        let left_neighbors = self.neighbors(handle, Direction::Left);
        let right_neighbors = self.neighbors(handle, Direction::Left);

        for other in left_neighbors {

        }
    }
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_node_record_indices() {
        let mut graph = PackedGraph::new();
        let id_0 = NodeId::from(5);
        let id_1 = NodeId::from(7);
        let id_2 = NodeId::from(3);
        let id_3 = NodeId::from(10);

        let _g_0 = graph.push_node_record(id_0);
        let _g_1 = graph.push_node_record(id_1);
        let _g_2 = graph.push_node_record(id_2);
        let _g_3 = graph.push_node_record(id_3);

        assert_eq!(graph.min_id, 3);
        assert_eq!(graph.max_id, 10);

        assert_eq!(graph.id_graph_map.get(2), 1);
        assert_eq!(graph.id_graph_map.get(4), 2);
        assert_eq!(graph.id_graph_map.get(0), 3);
        assert_eq!(graph.id_graph_map.get(7), 4);
    }

    #[test]
    fn packedgraph_create_handle() {
        let mut graph = PackedGraph::new();
        let n0 = NodeId::from(1);
        let n1 = NodeId::from(2);
        let n2 = NodeId::from(3);
        let h0 = graph.create_handle(b"GTCCA", n0);
        let h1 = graph.create_handle(b"AAA", n1);
        let h2 = graph.create_handle(b"GTGTGT", n2);

        let g0 = graph.handle_graph_ix(h0);
        assert_eq!(g0, Some(GraphIx(0)));

        let g1 = graph.handle_graph_ix(h1);
        assert_eq!(g1, Some(GraphIx(1)));

        let g2 = graph.handle_graph_ix(h2);
        assert_eq!(g2, Some(GraphIx(2)));

        let handles = vec![h0, h1, h2];

        use crate::handlegraph::{AllHandles, HandleGraph, HandleSequences};

        use bstr::{BString, B};

        let sequences = vec![B("GTCCA"), B("AAA"), B("GTGTGT")];

        let s0: BString = graph.sequence(h0).into();
        let s1: BString = graph.sequence(h1).into();
        let s2: BString = graph.sequence(h2).into();

        let s0_b: BString = graph.sequence_iter(h0).collect();
        let s1_b: BString = graph.sequence_iter(h1).collect();
        let s2_b: BString = graph.sequence_iter(h2).collect();

        assert_eq!(
            sequences,
            vec![s0.as_slice(), s1.as_slice(), s2.as_slice()]
        );

        assert_eq!(
            sequences,
            vec![s0_b.as_slice(), s1_b.as_slice(), s2_b.as_slice()]
        );

        let mut handles = graph.all_handles().collect::<Vec<_>>();
        handles.sort();
        let hnd = |x: u64| Handle::pack(x, false);
        assert_eq!(handles, vec![hnd(1), hnd(2), hnd(3)]);
    }

    #[test]
    fn packedgraph_create_edge() {
        use crate::handlegraph::{AllEdges, HandleNeighbors, HandleSequences};
        use bstr::{BString, B};

        let mut graph = PackedGraph::new();

        let seqs =
            vec![B("GTCCA"), B("AAA"), B("GTGTGT"), B("TTCT"), B("AGTAGT")];
        //   node     1          2           3           4          5

        let mut handles: Vec<Handle> = Vec::new();

        for seq in seqs.iter() {
            let handle = graph.append_handle(seq);
            handles.push(handle);
        }

        let hnd = |x: u64| Handle::pack(x, false);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(1, 3));

        graph.create_edge(edge(2, 4));

        graph.create_edge(edge(3, 4));
        graph.create_edge(edge(3, 5));

        graph.create_edge(edge(4, 5));

        let adj = |x: u64, left: bool| {
            let dir = if left {
                Direction::Left
            } else {
                Direction::Right
            };
            graph
                .neighbors(hnd(x), dir)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>()
        };

        assert!(adj(1, true).is_empty());
        assert_eq!(vec![3, 2], adj(1, false));

        assert_eq!(vec![1], adj(2, true));
        assert_eq!(vec![4], adj(2, false));

        assert_eq!(vec![1], adj(3, true));
        assert_eq!(vec![5, 4], adj(3, false));

        assert_eq!(vec![3, 2], adj(4, true));
        assert_eq!(vec![5], adj(4, false));

        assert_eq!(vec![4, 3], adj(5, true));
        assert!(adj(5, false).is_empty());

        let edges = graph.all_edges().collect::<Vec<_>>();

        assert_eq!(
            vec![
                edge(1, 3),
                edge(1, 2),
                edge(2, 4),
                edge(3, 5),
                edge(3, 4),
                edge(4, 5)
            ],
            edges
        );
    }

    #[test]
    fn packedgraph_remove_handle_edgelist() {
        use crate::handlegraph::HandleNeighbors;
        use bstr::B;

        let mut graph = PackedGraph::new();

        let seqs =
            vec![B("GTCCA"), B("AAA"), B("GTGTGT"), B("TTCT"), B("AGTAGT")];
        //   node     1          2           3           4          5

        let mut handles: Vec<Handle> = Vec::new();

        for seq in seqs.iter() {
            let handle = graph.append_handle(seq);
            handles.push(handle);
        }

        let hnd = |x: u64| Handle::pack(x, false);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        graph.create_edge(edge(1, 5));
        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(1, 3));
        graph.create_edge(edge(1, 4));

        let adj = |graph: &PackedGraph, x: u64, left: bool| {
            let dir = if left {
                Direction::Left
            } else {
                Direction::Right
            };
            graph
                .neighbors(hnd(x), dir)
                .map(|h| u64::from(h.id()))
                .collect::<Vec<_>>()
        };

        assert_eq!(vec![4, 3, 2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(3));
        assert_eq!(vec![4, 2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(4));
        assert_eq!(vec![2, 5], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(5));
        assert_eq!(vec![2], adj(&graph, 1, false));

        graph.remove_handle_from_edge(hnd(1), Direction::Right, hnd(2));
        assert!(adj(&graph, 1, false).is_empty());
    }

    #[test]
    fn packedgraph_split_sequence() {
        let mut graph = PackedGraph::new();
        let n0 = NodeId::from(1);
        let n1 = NodeId::from(2);
        let n2 = NodeId::from(3);
        let h0 = graph.create_handle(b"GTCCA", n0);
        let h1 = graph.create_handle(b"AAA", n1);
        let h2 = graph.create_handle(b"GTGTGT", n2);

        use crate::handlegraph::{AllHandles, HandleGraph, HandleSequences};

        use bstr::{BString, B};

        let hnd = |x: u64| Handle::pack(x, false);

        let seq_bstr = |g: &PackedGraph, h: u64| -> BString {
            g.sequence_iter(hnd(h)).collect()
        };

        assert_eq!(seq_bstr(&graph, 1), B("GTCCA"));
        assert_eq!(seq_bstr(&graph, 2), B("AAA"));
        assert_eq!(seq_bstr(&graph, 3), B("GTGTGT"));

        println!("{}", seq_bstr(&graph, 1));
        println!("{}", seq_bstr(&graph, 2));
        println!("{}", seq_bstr(&graph, 3));

        let s_ix_1 = graph.handle_graph_ix(hnd(1)).unwrap();
        let s_ix_1 = s_ix_1.to_seq_record_ix();
        println!("seq rec ix {}", s_ix_1);

        let splits = graph.sequences.divide_sequence(s_ix_1, vec![2, 3]);

        println!("{:?}", splits);

        // assert_eq!(seq_bstr(&graph, 1), B("GT"));
        // assert_eq!(seq_bstr(&graph, 2), B("AAA"));
        // assert_eq!(seq_bstr(&graph, 3), B("GTGTGT"));

        println!("{}", seq_bstr(&graph, 1));
        println!("{}", seq_bstr(&graph, 2));
        println!("{}", seq_bstr(&graph, 3));

        println!("all sequences");
        // let indices = graph.sequences.indices.iter().collect::<Vec<_>>();
        let indices = vec![0, 1, 2, 3, 4];
        for &ix in indices.iter() {
            let new_seq = graph.sequences.iter(ix, false).collect::<BString>();
            println!("{}", new_seq);
        }

        // let (ix, _len) = splits[0];
        // let new_seq = graph.sequences.iter(ix, false).collect::<BString>();
        // assert_eq!(new_seq, B("CCA"));
        // println!("{}", new_seq);
    }
}
