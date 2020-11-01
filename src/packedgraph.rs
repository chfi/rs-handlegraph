#![allow(dead_code)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_imports)]

use bio::alphabets::dna;
use bstr::{BString, ByteSlice};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::MutableHandleGraph,
};

pub mod edges;
pub mod graph;
pub mod iter;
pub mod nodes;
pub mod sequence;

pub use self::edges::{
    EdgeListIter, EdgeListIx, EdgeLists, EdgeRecord, EdgeVecIx,
};
pub use self::graph::PackedGraph;
use self::graph::{PackedSeqIter, SeqRecordIx, Sequences};
pub use self::iter::{EdgeListHandleIter, PackedHandlesIter};
pub use self::nodes::{GraphRecordIx, GraphVecIx, NodeIdIndexMap, NodeRecords};

impl<'a> AllHandles for &'a PackedGraph {
    type Handles = PackedHandlesIter<crate::packed::PackedDequeIter<'a>>;

    #[inline]
    fn all_handles(self) -> Self::Handles {
        let iter = self.nodes.nodes_iter();
        PackedHandlesIter::new(iter, usize::from(self.min_node_id()))
    }

    #[inline]
    fn node_count(self) -> usize {
        self.nodes.node_count()
    }

    #[inline]
    fn has_node<I: Into<NodeId>>(self, n_id: I) -> bool {
        self.nodes.has_node(n_id)
    }
}

impl<'a> AllEdges for &'a PackedGraph {
    type Edges = EdgesIter<&'a PackedGraph>;

    fn all_edges(self) -> Self::Edges {
        EdgesIter::new(self)
    }

    #[inline]
    fn edge_count(self) -> usize {
        self.edges.len()
    }
}

impl<'a> HandleNeighbors for &'a PackedGraph {
    type Neighbors = EdgeListHandleIter<'a>;

    #[inline]
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors {
        use Direction as Dir;
        let g_ix = self.nodes.handle_record(handle).unwrap();

        let edge_list_ix = match (dir, handle.is_reverse()) {
            (Dir::Left, true) => {
                self.nodes.get_edge_list(g_ix, Direction::Right)
            }
            (Dir::Left, false) => {
                self.nodes.get_edge_list(g_ix, Direction::Left)
            }
            (Dir::Right, true) => {
                self.nodes.get_edge_list(g_ix, Direction::Left)
            }
            (Dir::Right, false) => {
                self.nodes.get_edge_list(g_ix, Direction::Right)
            }
        };

        let iter = self.edges.iter(edge_list_ix);

        EdgeListHandleIter::new(iter)
    }
}

impl<'a> HandleSequences for &'a PackedGraph {
    type Sequence = PackedSeqIter<'a>;

    #[inline]
    fn sequence_iter(self, handle: Handle) -> Self::Sequence {
        let g_ix = self.nodes.handle_record(handle).unwrap();
        let seq_ix = SeqRecordIx::from_graph_record_ix(g_ix);
        self.nodes
            .sequences()
            .iter(seq_ix.unwrap(), handle.is_reverse())
    }

    #[inline]
    fn node_len(self, handle: Handle) -> usize {
        let g_ix = self.nodes.handle_record(handle).unwrap();
        self.nodes.sequences().length(g_ix)
    }
}

impl HandleGraph for PackedGraph {
    #[inline]
    fn min_node_id(&self) -> NodeId {
        self.nodes.min_id().into()
    }
    #[inline]
    fn max_node_id(&self) -> NodeId {
        self.nodes.max_id().into()
    }
}

impl<'a> HandleGraphRef for &'a PackedGraph {
    #[inline]
    fn total_length(self) -> usize {
        self.nodes.sequences().total_length()
    }
}

impl MutableHandleGraph for PackedGraph {
    fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = NodeId::from(self.max_node_id() + 1);
        self.create_handle(sequence, id)
    }

    fn create_handle<T: Into<NodeId>>(
        &mut self,
        sequence: &[u8],
        node_id: T,
    ) -> Handle {
        let id = node_id.into();
        assert!(
            id != NodeId::from(0)
                && !sequence.is_empty()
                && !self.nodes.has_node(id)
        );

        let g_ix = self.nodes.create_node(id, sequence).unwrap();

        Handle::pack(id, false)
    }

    fn create_edge(&mut self, Edge(left, right): Edge) {
        let left_gix = self.nodes.handle_record(left).unwrap();
        let right_gix = self.nodes.handle_record(right).unwrap();

        let left_edge_ix = if left.is_reverse() {
            left_gix.as_vec_ix().unwrap().left_edges_ix()
        } else {
            left_gix.as_vec_ix().unwrap().right_edges_ix()
        };

        let right_edge_ix = if right.is_reverse() {
            right_gix.as_vec_ix().unwrap().right_edges_ix()
        } else {
            right_gix.as_vec_ix().unwrap().left_edges_ix()
        };

        let left_edge_dir = if left.is_reverse() {
            Direction::Left
        } else {
            Direction::Right
        };

        let right_edge_dir = if right.is_reverse() {
            Direction::Right
        } else {
            Direction::Left
        };

        let left_edge_list = self.nodes.get_edge_list(left_gix, left_edge_dir);

        // create the record for the edge from the left handle to the right
        let left_to_right = self.edges.append_record(right, left_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the left handle
        self.nodes
            .set_edge_list(left_gix, left_edge_dir, left_to_right);
        // self.records_vec
        //     .set(left_edge_ix, left_to_right.as_vec_value());

        let right_edge_list =
            self.nodes.get_edge_list(right_gix, right_edge_dir);

        // create the record for the edge from the right handle to the left
        let right_to_left = self.edges.append_record(left, right_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the right handle

        self.nodes
            .set_edge_list(right_gix, right_edge_dir, right_to_left);
    }

    fn divide_handle(
        &mut self,
        handle: Handle,
        mut offsets: Vec<usize>,
    ) -> Vec<Handle> {
        let mut result = vec![handle];

        let node_len = self.node_len(handle);

        let fwd_handle = handle.forward();

        let mut lengths = Vec::with_capacity(offsets.len() + 1);

        let mut last_offset = 0;
        let mut total_len = 0;

        for &offset in offsets.iter() {
            let len = offset - last_offset;
            total_len += len;
            lengths.push(len);
            last_offset = offset;
        }

        if total_len < node_len {
            let len = node_len - total_len;
            lengths.push(len);
        }

        if handle.is_reverse() {
            lengths.reverse();
        }

        // let g_ix = self.nodes.handle_record(handle).unwrap();
        // let seq_ix = SeqRecordIx::from_graph_record_ix(g_ix).unwrap();

        let seq_ix = self
            .nodes
            .handle_record(handle)
            .and_then(SeqRecordIx::from_graph_record_ix)
            .unwrap();

        // Split the sequence and get the new sequence records
        let new_seq_ixs =
            self.nodes.sequences_mut().split_sequence(seq_ix, &lengths);

        if new_seq_ixs.is_none() {
            panic!(
                "Something went wrong when \
                 dividing the handle {:?} with offsets {:#?}",
                handle, offsets
            );
        }

        let new_seq_ixs = new_seq_ixs.unwrap();

        // Add new nodes and graph records for the new sequence records

        for &s_ix in new_seq_ixs.iter() {
            let n_id = self.nodes.append_empty_node();
            let h = Handle::pack(n_id, false);
            result.push(h);
        }

        let handle_gix = self.nodes.handle_record(handle).unwrap();

        let last_handle = *result.last().unwrap();
        let last_gix = self.nodes.handle_record(last_handle).unwrap();

        // Move the right-hand edges of the original handle to the
        // corresponding side of the new graph
        let old_right_record_edges =
            self.nodes.get_edge_list(handle_gix, Direction::Right);

        let new_right_edges_head = self.nodes.set_edge_list(
            last_gix,
            Direction::Right,
            old_right_record_edges,
        );

        // Remove the right-hand edges of the original handle
        self.nodes.set_edge_list(
            handle_gix,
            Direction::Right,
            EdgeListIx::empty(),
        );

        // Update back references for the nodes connected to the
        // right-hand side of the original handle

        // Get the edge lists with the back references
        let right_neighbors = self
            .neighbors(last_handle, Direction::Right)
            .map(|h| {
                let g_ix = self.nodes.handle_record(h).unwrap();
                self.nodes.get_edge_list(g_ix, Direction::Left)
            })
            .collect::<Vec<_>>();

        // Update the corresponding edge record in each of the
        // neighbor back reference lists
        for edge_list in right_neighbors {
            self.edges.update_edge_record(
                edge_list,
                |_, (h, _)| h == handle,
                |(_, n)| (last_handle, n),
            );
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
        if !handle.is_reverse() {
            return handle;
        }

        let edges = self
            .neighbors(handle, Direction::Left)
            .chain(self.neighbors(handle, Direction::Right))
            .collect::<Vec<_>>();

        handle.flip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_divide_handle() {
        use bstr::B;

        let mut graph = PackedGraph::new();
        let h1 = graph.append_handle(b"GTCA");
        let h2 = graph.append_handle(b"AAGTGCTAGT");
        let h3 = graph.append_handle(b"ATA");
        let h4 = graph.append_handle(b"AA");
        let h5 = graph.append_handle(b"GG");

        let hnd = |x: u64| Handle::pack(x, false);
        let r_hnd = |x: u64| Handle::pack(x, true);

        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));
        let r_edge = |l: u64, r: u64| Edge(r_hnd(l), r_hnd(r));

        let bseq =
            |g: &PackedGraph, x: u64| -> BString { g.sequence(hnd(x)).into() };

        /*
           1-
             \ /-----3
              2     /
             / \   /
           4-   -5-
        */

        graph.create_edge(edge(1, 2));
        graph.create_edge(edge(4, 2));

        graph.create_edge(edge(2, 3));
        graph.create_edge(edge(2, 5));

        graph.create_edge(edge(5, 3));

        let new_hs = graph.divide_handle(hnd(2), vec![3, 7, 9]);

        assert_eq!(bseq(&graph, 2), B("AAG"));

        let new_seqs: Vec<BString> =
            new_hs.iter().map(|h| graph.sequence(*h).into()).collect();

        // The sequence is correctly split
        assert_eq!(new_seqs, vec![B("AAG"), B("TGCT"), B("AG"), B("T")]);

        let mut edges = graph.all_edges().collect::<Vec<_>>();
        edges.sort();

        // The edges are all correct
        assert_eq!(
            edges,
            vec![
                edge(1, 2),
                edge(2, 6),
                r_edge(2, 4),
                r_edge(3, 5),
                r_edge(3, 8),
                r_edge(5, 8),
                edge(6, 7),
                edge(7, 8)
            ]
        );
    }
}
