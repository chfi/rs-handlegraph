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
        let seq_ix = SeqRecordIx::from_graph_record_ix(g_ix).unwrap();
        self.nodes.sequences().iter(seq_ix, handle.is_reverse())
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

        unimplemented!();

        /*
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

        let mut lengths = Vec::with_capacity(offsets.len());

        // for &offset in offsets.iter().skip(1) {
        for &offset in offsets.iter() {
            let step = if handle.is_reverse() {
                let v = last_ix - offset;
                last_ix = offset;
                v
            } else {
                let v = offset - last_ix;
                last_ix = offset;
                v
            };
            lengths.push(step);
            // let seq: Vec<u8> = seq_iter.by_ref().take(step).collect();
            // subseqs.push(seq);
        }

        let sec_ix = self.handle_graph_ix(handle).unwrap();
        let sec_ix = sec_ix.to_seq_record_ix();

        println!("{:?}", lengths);
        let subseq_recs = self.sequences.divide_sequence(sec_ix, lengths);
        println!("{:?}", subseq_recs);

        for &i in subseq_recs.iter() {
            let h = self.append_record_handle();
            result.push(h);
        }
        */

        /*
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
        */
        /*

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
            */
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
