use bio::alphabets::dna;
use bstr::BString;

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::{
        AllEdges, AllHandles, HandleGraph, HandleNeighbors, HandleSequences,
    },
};

pub mod graph;

use self::graph::{EdgeIx, EdgeLists, EdgeRecord, PackedSeqIter, Sequences};
pub use self::graph::{GraphIx, PackedGraph};

impl HandleGraph for PackedGraph {
    fn has_node(&self, id: NodeId) -> bool {
        self.get_node_index(id).is_some()
    }

    /// The length of the sequence of a given node
    fn length(&self, handle: Handle) -> usize {
        let graph_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = graph_ix.to_seq_record_ix();
        self.sequences.length(seq_ix)
    }

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    fn sequence(&self, handle: Handle) -> Vec<u8> {
        self.sequence_iter(handle).collect()
    }

    fn subsequence(
        &self,
        handle: Handle,
        index: usize,
        size: usize,
    ) -> Vec<u8> {
        self.sequence_iter(handle).skip(index).take(size).collect()
    }

    fn base(&self, handle: Handle, index: usize) -> u8 {
        let g_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = g_ix.to_seq_record_ix();
        self.sequences.base(seq_ix, index)
    }

    #[inline]
    fn min_node_id(&self) -> NodeId {
        self.min_id.into()
    }
    #[inline]
    fn max_node_id(&self) -> NodeId {
        self.max_id.into()
    }

    /// Return the total number of nodes in the graph
    #[inline]
    fn node_count(&self) -> usize {
        // need to make sure this is correct, especially once I add deletion
        self.id_graph_map.len()
    }

    /// Return the total number of edges in the graph
    #[inline]
    fn edge_count(&self) -> usize {
        self.edges.edge_lists.len() / EdgeLists::RECORD_SIZE
        // need to make sure this is correct!
    }

    /// Sum up all the sequences in the graph
    fn total_length(&self) -> usize {
        self.sequences.total_length()
    }

    fn handle_edges_iter<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> Box<dyn Iterator<Item = Handle> + 'a> {
        Box::new(self.neighbors(handle, dir))
    }

    fn handles_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Handle> + 'a> {
        Box::new(self.all_handles())
    }

    fn edges_iter<'a>(&'a self) -> Box<dyn Iterator<Item = Edge> + 'a> {
        unimplemented!();
    }
}

/// Iterator over a PackedGraph's handles. For every non-zero value in
/// the PackedDeque holding the PackedGraph's node ID mappings, the
/// corresponding index is mapped back to the original ID and yielded
/// by the iterator.
pub struct PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    iter: std::iter::Enumerate<I>,
    min_id: usize,
}

impl<I> PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    fn new(iter: I, min_id: usize) -> Self {
        let iter = iter.enumerate();
        Self { iter, min_id }
    }
}

impl<I> Iterator for PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        while let Some((ix, id)) = self.iter.next() {
            if id != 0 {
                let n_id = ix + self.min_id;
                return Some(Handle::pack(n_id, false));
            }
        }
        None
    }
}

impl<'a> AllHandles for &'a PackedGraph {
    type Handles = PackedHandlesIter<crate::packed::PackedDequeIter<'a>>;

    #[inline]
    fn all_handles(self) -> Self::Handles {
        let iter = self.id_graph_map.iter();
        PackedHandlesIter::new(iter, self.min_id as usize)
    }
}

/// Iterator for stepping through an edge list, returning Handles.
pub struct EdgeListIter<'a> {
    edge_lists: &'a EdgeLists,
    current: EdgeRecord,
    finished: bool,
}

impl<'a> EdgeListIter<'a> {
    fn new(graph: &'a PackedGraph, start: EdgeIx) -> Self {
        let edge_lists = &graph.edges;
        let current = edge_lists.get_record(start);
        Self {
            edge_lists,
            current,
            finished: false,
        }
    }
}

impl<'a> Iterator for EdgeListIter<'a> {
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        if self.finished {
            return None;
        }
        let item = self.current.handle;
        if let Some(next) = self.edge_lists.next(self.current) {
            self.current = next;
            Some(item)
        } else {
            self.finished = true;
            None
        }
    }
}

impl<'a> HandleNeighbors for &'a PackedGraph {
    type Neighbors = EdgeListIter<'a>;

    #[inline]
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors {
        use Direction as Dir;
        let g_ix = self.handle_graph_ix(handle).unwrap();

        let edge_list_ix = match (dir, handle.is_reverse()) {
            (Dir::Left, true) => g_ix.end_edges_ix(),
            (Dir::Left, false) => g_ix.start_edges_ix(),
            (Dir::Right, true) => g_ix.start_edges_ix(),
            (Dir::Right, false) => g_ix.end_edges_ix(),
        };

        let edge_ix = self.get_edge_list_ix(edge_list_ix);

        EdgeListIter::new(self, edge_ix)
    }
}

impl<'a> HandleSequences for &'a PackedGraph {
    type Sequence = PackedSeqIter<'a>;

    #[inline]
    fn sequence_iter(self, handle: Handle) -> Self::Sequence {
        let g_ix = self.handle_graph_ix(handle).unwrap();
        let seq_ix = g_ix.to_seq_record_ix();
        self.sequences.iter(seq_ix, handle.is_reverse())
    }
}
