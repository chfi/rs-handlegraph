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
pub use super::nodes::{
    GraphRecordIx, GraphVecIx, NodeIdIndexMap, NodeRecords,
};
pub use super::sequence::{PackedSeqIter, SeqRecordIx, Sequences};

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub(super) nodes: NodeRecords,
    pub(super) edges: EdgeLists,
}

impl Default for PackedGraph {
    fn default() -> Self {
        let nodes = Default::default();
        let edges = Default::default();
        PackedGraph { nodes, edges }
    }
}

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    /*
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
    */
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
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
    */

    /*
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

    */
}
