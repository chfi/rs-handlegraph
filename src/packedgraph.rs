use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{AdditiveHandleGraph, MutableHandleGraph},
};

pub mod edges;
pub mod graph;
pub mod index;
pub mod iter;
pub mod nodes;
pub mod paths;
pub mod sequence;

pub use self::{
    edges::{EdgeListIx, EdgeLists, EdgeRecord, EdgeVecIx},
    graph::PackedGraph,
    index::*,
    iter::{EdgeListHandleIter, PackedHandlesIter},
    nodes::{GraphRecordIx, GraphVecIx, NodeIdIndexMap, NodeRecords},
    paths::*,
};

use self::graph::{PackedSeqIter, SeqRecordIx};

use crate::packed;

impl<'a> AllHandles for &'a PackedGraph {
    type Handles = PackedHandlesIter<packed::deque::Iter<'a>>;

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
        let rec_id = self.nodes.handle_record(handle).unwrap();
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id);
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

impl AdditiveHandleGraph for PackedGraph {
    fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = self.max_node_id() + 1;
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

        let _g_ix = self.nodes.create_node(id, sequence).unwrap();

        Handle::pack(id, false)
    }

    fn create_edge(&mut self, Edge(left, right): Edge) {
        let left_gix = self.nodes.handle_record(left).unwrap();
        let right_gix = self.nodes.handle_record(right).unwrap();

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
}

impl MutableHandleGraph for PackedGraph {
    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: Vec<usize>,
    ) -> Vec<Handle> {
        let mut result = vec![handle];

        let node_len = self.node_len(handle);

        let _fwd_handle = handle.forward();

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
            .and_then(SeqRecordIx::from_one_based_ix)
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

        for _s_ix in new_seq_ixs.iter() {
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

        self.nodes.set_edge_list(
            last_gix,
            Direction::Right,
            old_right_record_edges,
        );

        // Remove the right-hand edges of the original handle
        self.nodes.set_edge_list(
            handle_gix,
            Direction::Right,
            EdgeListIx::null(),
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
            if let [this, next] = *window {
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

        let g_ix = self.nodes.handle_record(handle).unwrap();

        // Overwrite the sequence with its reverse complement
        let rev_seq = self.sequence(handle);
        self.nodes
            .sequences_mut()
            .overwrite_sequence(g_ix, &rev_seq);

        // Flip the handle on the incoming edges
        let edges = self
            .neighbors(handle, Direction::Left)
            .chain(self.neighbors(handle, Direction::Right))
            .collect::<Vec<_>>();

        for target in edges {
            let tgt_g_ix = self.nodes.handle_record(target).unwrap();
            let backward_edge_list = if target.is_reverse() {
                self.nodes.get_edge_list(tgt_g_ix, Direction::Right)
            } else {
                self.nodes.get_edge_list(tgt_g_ix, Direction::Left)
            };

            self.edges.update_edge_record(
                backward_edge_list,
                |_, (h, _)| h == handle,
                |(h, n)| (h.flip(), n),
            );
        }

        // Swap the left and right edges on the handle
        self.nodes
            .update_node_edge_lists(g_ix, |l, r| (r, l))
            .unwrap();

        // TODO update paths and occurrences once they're implmented

        handle.flip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packedgraph_divide_handle() {
        use bstr::{BString, B};

        let mut graph = PackedGraph::new();
        let _h1 = graph.append_handle(b"GTCA");
        let _h2 = graph.append_handle(b"AAGTGCTAGT");
        let _h3 = graph.append_handle(b"ATA");
        let _h4 = graph.append_handle(b"AA");
        let _h5 = graph.append_handle(b"GG");

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
