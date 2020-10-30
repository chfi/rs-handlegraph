use bio::alphabets::dna;
use bstr::{BString, ByteSlice};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::{
        AllEdges, AllHandles, HandleGraph, HandleNeighbors, HandleSequences,
    },
    mutablehandlegraph::MutableHandleGraph,
};

pub mod graph;

use self::graph::{EdgeIx, EdgeLists, EdgeRecord, PackedSeqIter, Sequences};
pub use self::graph::{GraphIx, PackedGraph};

impl HandleGraph for PackedGraph {
    #[inline]
    fn has_node(&self, id: NodeId) -> bool {
        self.get_node_index(id).is_some()
    }

    /// The length of the sequence of a given node
    #[inline]
    fn length(&self, handle: Handle) -> usize {
        self.sequence_iter(handle).count()
    }

    /// Returns the sequence of a node in the handle's local forward
    /// orientation. Copies the sequence, as the sequence in the graph
    /// may be reversed depending on orientation.
    #[inline]
    fn sequence(&self, handle: Handle) -> Vec<u8> {
        self.sequence_iter(handle).collect()
    }

    #[inline]
    fn subsequence(
        &self,
        handle: Handle,
        index: usize,
        size: usize,
    ) -> Vec<u8> {
        self.sequence_iter(handle).skip(index).take(size).collect()
    }

    #[inline]
    fn base(&self, handle: Handle, index: usize) -> u8 {
        self.sequence_iter(handle).nth(index).unwrap()
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

    fn degree(&self, handle: Handle, dir: Direction) -> usize {
        self.neighbors(handle, dir).fold(0, |a, _| a + 1)
    }

    fn has_edge(&self, left: Handle, right: Handle) -> bool {
        self.neighbors(left, Direction::Right).any(|h| h == right)
    }
}

impl MutableHandleGraph for PackedGraph {
    fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = NodeId::from(self.max_id + 1);
        self.create_handle(sequence, id)
    }

    fn create_handle<T: Into<NodeId>>(
        &mut self,
        sequence: &[u8],
        node_id: T,
    ) -> Handle {
        let id = node_id.into();
        assert!(
            id != NodeId::from(0) && !sequence.is_empty() && !self.has_node(id)
        );

        let graph_ix = self.push_node_record(id);
        let seq_ix = graph_ix.to_seq_record_ix();

        self.sequences.add_record(seq_ix, sequence);

        Handle::pack(id, false)
    }

    fn create_edge(&mut self, Edge(left, right): Edge) {
        let left_g_ix = self.handle_graph_ix(left).unwrap();
        let right_g_ix = self.handle_graph_ix(right).unwrap();

        let left_edge_g_ix = if left.is_reverse() {
            left_g_ix.left_edges_ix()
        } else {
            left_g_ix.right_edges_ix()
        };

        let right_edge_g_ix = if right.is_reverse() {
            right_g_ix.right_edges_ix()
        } else {
            right_g_ix.left_edges_ix()
        };

        let right_next = self.get_edge_list_ix(left_edge_g_ix);
        let edge_ix = self.edges.append_record(right, right_next);

        self.graph_records.set(left_edge_g_ix, edge_ix.0 as u64);

        if left_edge_g_ix == right_edge_g_ix {
            // todo reversing self edge records?
            return;
        }

        let left_next = self.get_edge_list_ix(right_edge_g_ix);
        let edge_ix = self.edges.append_record(left.flip(), left_next);

        self.graph_records.set(right_edge_g_ix, edge_ix.0 as u64);
    }

    fn divide_handle(
        &mut self,
        handle: Handle,
        mut offsets: Vec<usize>,
    ) -> Vec<Handle> {
        let mut result = vec![handle];
        let node_len = self.length(handle);

        let fwd_handle = handle.forward();

        let seq_iter = self.sequence_iter(fwd_handle);

        let mut subseqs: Vec<Vec<u8>> = Vec::with_capacity(offsets.len() + 1);

        offsets.push(seq_iter.len());

        let mut last_ix = if handle.is_reverse() {
            seq_iter.len()
        } else {
            0
        };

        let mut seq_iter = seq_iter;

        for &offset in offsets.iter().skip(1) {
            let step = if handle.is_reverse() {
                let v = last_ix - offset;
                last_ix = offset;
                v
            } else {
                let v = offset - last_ix;
                last_ix = offset;
                v
            };
            println!("taking {} bases", step);
            let seq: Vec<u8> = seq_iter.by_ref().take(step).collect();
            println!("seq length: {}", seq.len());
            println!("pushing sequence {}", seq.as_bstr());
            subseqs.push(seq);
        }

        // ensure the order of the subsequences is correct if the
        // handle was reversed
        if handle.is_reverse() {
            subseqs.reverse();
        }

        for mut seq in subseqs {
            let h = self.append_handle(&seq);
            result.push(h);
        }

        let g_ix = self.handle_graph_ix(handle).unwrap();
        self.sequences
            .lengths
            .set(g_ix.to_seq_record_ix(), offsets[0] as u64);

        // Move the right-hand edges of the original handle to the
        // corresponding side of the new graph
        let old_right_edges_ix =
            self.handle_edge_record_ix(handle, Direction::Right);
        let new_right_edges_ix = self
            .handle_edge_record_ix(*result.last().unwrap(), Direction::Right);

        self.swap_graph_record_elements(old_right_edges_ix, new_right_edges_ix);

        // Update back references for the nodes connected to the
        // right-hand side of the original handle
        let right_neighbors = self
            .neighbors(handle, Direction::Right)
            .map(|h| self.handle_edge_record_ix(h, Direction::Left))
            .collect::<Vec<_>>();

        let new_right_edge = self.graph_records.get(new_right_edges_ix);
        for ix in right_neighbors {
            self.graph_records.set(ix, new_right_edge);
        }

        // create edges between the new segments
        for window in result.windows(2) {
            if let &[this, next] = window {
                self.create_edge(Edge(this, next));
            }
        }

        // TODO update paths and occurrences once they're implmented

        result
    }

    fn apply_orientation(&mut self, handle: Handle) -> Handle {
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
pub struct EdgeListHandleIter<'a> {
    edge_lists: &'a EdgeLists,
    current: Option<EdgeRecord>,
    finished: bool,
}

impl<'a> EdgeListHandleIter<'a> {
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

impl<'a> Iterator for EdgeListHandleIter<'a> {
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        if self.finished {
            return None;
        }
        if let Some(current) = self.current {
            let item = current.handle;
            self.current = self.edge_lists.next(current);
            Some(item)
        } else {
            self.finished = true;
            None
        }
    }
}

impl<'a> HandleNeighbors for &'a PackedGraph {
    type Neighbors = EdgeListHandleIter<'a>;

    #[inline]
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors {
        use Direction as Dir;
        let g_ix = self.handle_graph_ix(handle).unwrap();

        let edge_list_ix = match (dir, handle.is_reverse()) {
            (Dir::Left, true) => g_ix.right_edges_ix(),
            (Dir::Left, false) => g_ix.left_edges_ix(),
            (Dir::Right, true) => g_ix.left_edges_ix(),
            (Dir::Right, false) => g_ix.right_edges_ix(),
        };

        let edge_ix = self.get_edge_list_ix(edge_list_ix);

        EdgeListHandleIter::new(self, edge_ix)
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

use crate::handlegraph::iter::EdgesIter;

impl<'a> AllEdges for &'a PackedGraph {
    type Edges = EdgesIter<&'a PackedGraph>;

    fn all_edges(self) -> Self::Edges {
        EdgesIter::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_divide_handle() {
        let mut graph = PackedGraph::new();
        graph.append_handle(b"GTCA");
        graph.append_handle(b"AAGTGCTAGT");
        graph.append_handle(b"ATA");

        println!("before split");
        for h in graph.all_handles() {
            let seq: BString = graph.sequence(h).into();
            println!("{:?}\t{}", h.id(), seq);
        }

        let hnd = |x: u64| Handle::pack(x, false);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(2, 3));

        let new_hs = graph.divide_handle(hnd(2), vec![3, 7, 9]);

        println!("after split");
        println!("{:?}", new_hs);

        for Edge(l, r) in graph.all_edges() {
            println!("{:?}\t{:?}", l.id(), r.id());
        }

        for h in graph.all_handles() {
            let seq: BString = graph.sequence(h).into();
            println!("{:?}\t{}", h.id(), seq);
        }
    }
}
