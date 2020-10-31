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

use super::edges::{EdgeListIter, EdgeListIx, EdgeLists, EdgeVecIx};
use super::graph::{NARROW_PAGE_WIDTH, WIDE_PAGE_WIDTH};
use super::sequence::{PackedSeqIter, SeqRecordIx, Sequences};

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
    pub(super) fn new<I: Into<usize>>(x: I) -> Self {
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

    #[inline]
    pub(super) fn seq_record_ix(&self) -> usize {
        self.0 / 2
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

    pub(super) fn iter(&self) -> PackedDequeIter<'_> {
        self.deque.iter()
    }

    pub(super) fn len(&self) -> usize {
        self.deque.len()
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

    #[inline]
    fn has_node<I: Into<NodeId>>(&self, id: I) -> bool {
        self.get_index(id).is_some()
    }

    #[inline]
    fn get_index<I: Into<NodeId>>(&self, id: I) -> Option<GraphRecordIx> {
        let id = u64::from(id.into());
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let index = u64::from(id) - self.min_id;
        let value = self.deque.get(index as usize);
        let rec_ix = GraphRecordIx::from_vec_value(value);
        Some(rec_ix)
    }
}

#[derive(Debug, Clone)]
pub struct NodeRecords {
    records_vec: PagedIntVec,
    id_index_map: NodeIdIndexMap,
    sequences: Sequences,
    removed_nodes: Vec<NodeId>,
}

impl Default for NodeRecords {
    fn default() -> NodeRecords {
        Self {
            records_vec: PagedIntVec::new(NARROW_PAGE_WIDTH),
            id_index_map: Default::default(),
            sequences: Default::default(),
            removed_nodes: Vec::new(),
        }
    }
}

impl NodeRecords {
    #[inline]
    pub fn min_id(&self) -> u64 {
        self.id_index_map.min_id
    }

    #[inline]
    pub fn max_id(&self) -> u64 {
        self.id_index_map.max_id
    }

    pub fn nodes_iter(&self) -> PackedDequeIter<'_> {
        self.id_index_map.iter()
    }

    #[inline]
    pub fn has_node<I: Into<NodeId>>(&self, id: I) -> bool {
        self.id_index_map.has_node(id)
    }

    #[inline]
    pub fn node_count(&self) -> usize {
        self.id_index_map.len()
    }

    #[inline]
    pub fn total_length(&self) -> usize {
        self.sequences.total_length()
    }

    /// Return the `GraphRecordIx` that will be used by the next node
    /// that's inserted into the graph.
    fn next_graph_ix(&self) -> GraphRecordIx {
        let rec_count = self.records_vec.len();
        let vec_ix = GraphVecIx::new(rec_count);
        vec_ix.as_record_ix()
    }

    pub(super) fn sequences(&self) -> &Sequences {
        &self.sequences
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
        if n_id == NodeId::from(0) {
            return None;
        }

        let next_ix = self.next_graph_ix();

        // Make sure the sequences and graph record indices are synced
        if self.sequences.expected_next_record() != next_ix {
            return None;
        }

        // Make sure the node ID is valid and doesn't already exist
        if !self.id_index_map.append_node_id(n_id, next_ix) {
            return None;
        }

        // append the sequence and graph records
        self.sequences.append_empty_record(next_ix);
        let record_ix = self.append_node_graph_record(next_ix)?;

        Some(record_ix)
    }

    #[inline]
    pub(super) fn get_edge_list(
        &self,
        g_ix: GraphRecordIx,
        dir: Direction,
    ) -> EdgeListIx {
        match g_ix.as_vec_ix() {
            None => EdgeListIx::empty(),
            Some(vec_ix) => {
                let ix = match dir {
                    Direction::Right => vec_ix.right_edges_ix(),
                    Direction::Left => vec_ix.left_edges_ix(),
                };

                EdgeListIx::from_vec_value(self.records_vec.get(ix))
            }
        }
    }

    #[inline]
    pub(super) fn set_edge_list(
        &mut self,
        g_ix: GraphRecordIx,
        dir: Direction,
        new_edge: EdgeListIx,
    ) -> Option<()> {
        let vec_ix = g_ix.as_vec_ix()?;

        let ix = match dir {
            Direction::Right => vec_ix.right_edges_ix(),
            Direction::Left => vec_ix.left_edges_ix(),
        };

        self.records_vec.set(ix, new_edge.as_vec_value());
        Some(())
    }

    #[inline]
    pub(super) fn get_node_edge_lists(
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
        let (left_rec, right_rec) = self.get_node_edge_lists(g_ix)?;

        let (new_left, new_right) = f(left_rec, right_rec);

        let left = vec_ix.left_edges_ix();
        let right = vec_ix.right_edges_ix();
        self.records_vec.set(left, new_left.as_vec_value());
        self.records_vec.set(right, new_right.as_vec_value());
        Some(())
    }

    pub(super) fn create_node<I: Into<NodeId>>(
        &mut self,
        n_id: I,
        seq: &[u8],
    ) -> Option<GraphRecordIx> {
        let n_id = n_id.into();
        // update the node ID/graph index map
        let g_ix = self.insert_node(n_id)?;

        // insert the sequence
        let s_ix = self.sequences.append_sequence(g_ix, seq)?;

        Some(g_ix)
    }

    #[inline]
    pub(super) fn handle_record(&self, h: Handle) -> Option<GraphRecordIx> {
        self.id_index_map.get_index(h.id())
    }

    /*
    pub(super) fn insert_edge(
        &mut self,
        left: Handle,
        right: Handle,
    ) -> Option<()> {
        let left_gix = self.handle_record(left)?;
        let right_gix = self.handle_record(right)?;

        let left_edge_ix = if left.is_reverse() {
            left_gix.as_vec_ix().left_edges_ix()
        } else {
            left_gix.as_vec_ix().right_edges_ix()
        };

        let right_edge_ix = if right.is_reverse() {
            right_gix.as_vec_ix().right_edges_ix()
        } else {
            right_gix.as_vec_ix().left_edges_ix()
        };

        let left_edge_list =
            EdgeListIx::from_vec_value(self.records_vec.get(left_edge_ix));

        // create the record for the edge from the left handle to the right
        let left_to_right = self.edges.append_record(right, left_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the left handle
        self.records_vec
            .set(left_edge_ix, left_to_right.as_vec_value());

        let right_edge_list =
            EdgeListIx::from_vec_value(self.records_vec.get(right_edge_ix));

        // create the record for the edge from the right handle to the left
        let right_to_left = self.edges.append_record(left, right_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the right handle

        self.records_vec
            .set(right_edge_ix, right_to_left.as_vec_value());
    }
    */
}
