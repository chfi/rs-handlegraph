use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{
        AdditiveHandleGraph, MutableHandleGraph, SubtractiveHandleGraph,
    },
};

#[allow(unused_imports)]
use crate::pathhandlegraph::{
    AllPathIds, AllPathRefs, AllPathRefsMut, EmbeddedPaths, HandleOccurrences,
    MutEmbeddedPaths, MutHandleOccurrences, OccurBase, PathId, PathNames,
    PathNamesMut, PathRef, PathRefMut, PathRefs, PathRefsMut,
};

pub mod edges;
pub mod graph;
pub mod index;
pub mod iter;
pub mod nodes;
pub mod occurrences;
pub mod paths;
pub mod sequence;

mod defragment;
use defragment::Defragment;

pub use self::{
    edges::{EdgeListIx, EdgeLists, EdgeRecord, EdgeVecIx},
    graph::PackedGraph,
    index::{
        list, NodeRecordId, OneBasedIndex, PackedDoubleList, PackedList,
        PackedListMut, RecordIndex,
    },
    iter::{EdgeListHandleIter, PackedHandlesIter},
    nodes::{GraphVecIx, NodeIdIndexMap, NodeRecords},
    occurrences::{NodeOccurrences, OccurListIx, OccurRecord, OccurrencesIter},
    paths::*,
    sequence::{PackedSeqIter, Sequences},
};

use self::graph::SeqRecordIx;

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

impl OccurBase for PackedGraph {
    type StepIx = PathStepIx;
}

impl<'a> HandleOccurrences for &'a PackedGraph {
    type OccurIter = OccurrencesIter<'a>;

    fn handle_occurrences(self, handle: Handle) -> Self::OccurIter {
        let occ_ix = self.nodes.handle_occur_record(handle).unwrap();
        let iter = self.occurrences.iter(occ_ix);
        OccurrencesIter::new(iter)
    }
}

impl<'a> MutHandleOccurrences for &'a mut PackedGraph {
    fn apply_update(self, path_id: PathId, step: StepUpdate) {
        self.apply_node_occurrence(path_id, step)
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

impl SubtractiveHandleGraph for PackedGraph {
    fn remove_handle(&mut self, handle: Handle) -> bool {
        self.remove_handle_impl(handle).is_some()
    }

    fn remove_edge(&mut self, edge: Edge) -> bool {
        self.remove_edge_impl(edge).is_some()
    }

    fn clear_graph(&mut self) {
        unimplemented!();
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

impl MutEmbeddedPaths for PackedGraph {
    fn create_path(&mut self, name: &[u8], _circular: bool) -> PathId {
        self.paths.create_path(name)
    }

    fn remove_path(&mut self, id: PathId) {
        self.remove_path_impl(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::packed::PackedElement;

    fn hnd(x: u64) -> Handle {
        Handle::pack(x, false)
    }

    // returns the occurrence list for the provided node as a vector
    // of tuples in the format (PathId, StepIx)
    fn get_occurs(graph: &PackedGraph, id: u64) -> Vec<(u64, u64)> {
        let oc_ix = graph.nodes.handle_occur_record(hnd(id)).unwrap();
        let oc_iter = graph.occurrences.iter(oc_ix);
        oc_iter
            .map(|(_occ_ix, record)| (record.path_id.0, record.offset.pack()))
            .collect::<Vec<_>>()
    }

    fn test_graph_with_paths() -> PackedGraph {
        let hnd = |x: u64| Handle::pack(x, false);
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();
        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        use bstr::B;

        let mut graph = PackedGraph::new();

        let seqs = vec![
            //                  Node
            B("GTCA"),       //  1
            B("AAGTGCTAGT"), //  2
            B("ATA"),        //  3
            B("AGTA"),       //  4
            B("GTCCA"),      //  5
            B("GGGT"),       //  6
            B("AACT"),       //  7
            B("AACAT"),      //  8
            B("AGCC"),       //  9
        ];
        /*
        1 ----- 8 --- 4 -----
          \   /   \     \     \
            2      \     \      6
          /   \     \     \   /
        5 ----- 7 --- 3 --- 9
        */

        let _handles = seqs
            .iter()
            .map(|seq| graph.append_handle(seq))
            .collect::<Vec<_>>();

        macro_rules! insert_edges {
            ($graph:ident, [$(($from:literal, $to:literal)),*]) => {
                $(
                    $graph.create_edge(edge($from, $to));
                )*
            };
        }

        insert_edges!(
            graph,
            [
                (1, 2),
                (1, 8),
                (5, 2),
                (5, 7),
                (2, 8),
                (2, 7),
                (7, 3),
                (8, 3),
                (8, 4),
                (3, 9),
                (4, 9),
                (4, 6),
                (9, 6)
            ]
        );
        /* Paths
                path_1: 1 8 4 6
                path_2: 5 2 8 4 6
                path_3: 1 2 8 4 9 6
                path_4: 5 7 3 9 6
        */

        let prep_path =
            |graph: &mut PackedGraph, name: &[u8], steps: Vec<u64>| {
                let path = graph.paths.create_path(name);
                let hnds = vec_hnd(steps);
                (path, hnds)
            };

        let (_path_1, p_1_steps) =
            prep_path(&mut graph, b"path1", vec![1, 8, 4, 6]);

        let (_path_2, p_2_steps) =
            prep_path(&mut graph, b"path2", vec![5, 2, 8, 4, 6]);

        let (_path_3, p_3_steps) =
            prep_path(&mut graph, b"path3", vec![1, 2, 8, 4, 9, 6]);

        let (_path_4, p_4_steps) =
            prep_path(&mut graph, b"path4", vec![5, 7, 3, 9, 6]);

        let steps_vecs = vec![p_1_steps, p_2_steps, p_3_steps, p_4_steps];

        graph.zip_all_paths_mut_ctx(
            steps_vecs.into_iter(),
            |steps, _path_id, path| {
                steps
                    .into_iter()
                    .map(|h| path.append_handle(h))
                    .collect::<Vec<_>>()
            },
        );

        graph
    }
    #[test]
    fn removing_nodes() {
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();

        let path_steps = |graph: &PackedGraph, id: PathId| {
            let path_ref = graph.paths.path_ref(id).unwrap();

            path_ref
                .steps()
                .map(|(_step_ix, step)| u64::from(step.handle.id()))
                .collect::<Vec<_>>()
        };

        let get_all_neighbors =
            |graph: &PackedGraph, handles: &[Handle], dir: Direction| {
                handles
                    .iter()
                    .copied()
                    .map(|h| {
                        let id = u64::from(h.id());
                        (
                            id,
                            graph
                                .neighbors(h, dir)
                                .map(|h| u64::from(h.id()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>()
            };

        let mut graph = test_graph_with_paths();

        // removing node 2 should affect the edges of nodes 1, 5, 8, 7,
        // and remove path_2 (id 1) and path_3 (id 2)

        let unaffected_left_edges = vec_hnd(vec![1, 3, 4, 5, 6, 9]);
        let unaffected_right_edges = vec_hnd(vec![3, 4, 6, 7, 8, 9]);

        let affected_left_edges = vec_hnd(vec![7, 8]);
        let affected_right_edges = vec_hnd(vec![1, 5]);

        let unaffected_left_pre =
            get_all_neighbors(&graph, &unaffected_left_edges, Direction::Left);
        let unaffected_right_pre = get_all_neighbors(
            &graph,
            &unaffected_right_edges,
            Direction::Right,
        );

        let _path_ids =
            graph.paths.path_names.all_path_ids().collect::<Vec<_>>();

        let path_1 = graph.paths.path_names.get_path_id(b"path1").unwrap();
        let path_4 = graph.paths.path_names.get_path_id(b"path4").unwrap();

        let steps_1_pre = path_steps(&graph, path_1);
        let steps_4_pre = path_steps(&graph, path_4);

        assert!(graph.has_node(NodeId::from(2)));

        // remove the node
        graph.remove_handle(hnd(2));

        assert!(!graph.has_node(NodeId::from(2)));

        let unaffected_left_post =
            get_all_neighbors(&graph, &unaffected_left_edges, Direction::Left);
        let unaffected_right_post = get_all_neighbors(
            &graph,
            &unaffected_right_edges,
            Direction::Right,
        );

        let affected_left_post =
            get_all_neighbors(&graph, &affected_left_edges, Direction::Left);
        let affected_right_post =
            get_all_neighbors(&graph, &affected_right_edges, Direction::Right);

        let steps_1_post = path_steps(&graph, path_1);
        let steps_4_post = path_steps(&graph, path_4);

        // The unaffected nodes have the same edges
        assert_eq!(unaffected_left_pre, unaffected_left_post);
        assert_eq!(unaffected_right_pre, unaffected_right_post);

        // The affected nodes do not have any edge to 2
        assert_eq!(affected_left_post, vec![(7, vec![5]), (8, vec![1])]);
        assert_eq!(affected_right_post, vec![(1, vec![8]), (5, vec![7])]);

        // The paths that did not include 2 still exist and are the same
        assert_eq!(steps_1_pre, steps_1_post);
        assert_eq!(steps_4_pre, steps_4_post);

        let path_2 = graph.paths.path_names.get_path_id(b"path2");
        let path_3 = graph.paths.path_names.get_path_id(b"path3");

        // The paths that did include 2 have been deleted
        assert!(path_2.is_none());
        assert!(path_3.is_none());
    }

    #[test]
    fn removing_edges() {
        let mut graph = test_graph_with_paths();

        let get_neighbors = |graph: &PackedGraph, x: u64| {
            let left =
                graph.neighbors(hnd(x), Direction::Left).collect::<Vec<_>>();
            let right = graph
                .neighbors(hnd(x), Direction::Right)
                .collect::<Vec<_>>();
            (left, right)
        };

        let nbors_9 = get_neighbors(&graph, 9);
        let nbors_6 = get_neighbors(&graph, 6);

        // remove the edge (9, 6)
        let edge = Edge(hnd(9), hnd(6));

        graph.remove_edge(edge);

        let nbors_post_9 = get_neighbors(&graph, 9);
        let nbors_post_6 = get_neighbors(&graph, 6);

        // node 9's left edges are the same
        assert_eq!(nbors_9.0, nbors_post_9.0);

        // node 6's right edges are the same
        assert_eq!(nbors_6.1, nbors_post_6.1);

        // node 9 only had one right edge
        assert!(nbors_post_9.1.is_empty());

        // node 6's only left edge is now to node 4
        assert_eq!(nbors_post_6.0, vec![hnd(4)]);
    }

    #[test]
    fn add_remove_paths() {
        let mut graph = test_graph_with_paths();

        let _node_7_occ = get_occurs(&graph, 7);
        let node_8_occ = get_occurs(&graph, 8);

        let path_3 = graph.paths.path_names.get_path_id(b"path3").unwrap();
        let path_4 = graph.paths.path_names.get_path_id(b"path4").unwrap();

        graph.remove_path(path_4);

        let node_7_occ_1 = get_occurs(&graph, 7);
        let node_8_occ_1 = get_occurs(&graph, 8);

        assert!(node_7_occ_1.is_empty());
        assert_eq!(node_8_occ, node_8_occ_1);

        graph.remove_path(path_3);

        let node_8_occ_2 = get_occurs(&graph, 8);

        assert_eq!(node_8_occ_2, vec![(1, 3), (0, 2)]);
    }

    #[test]
    fn packedgraph_mutate_paths() {
        let hnd = |x: u64| Handle::pack(x, false);
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();
        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        use bstr::B;

        let mut graph = PackedGraph::new();

        let seqs = vec![
            //                  Node
            B("GTCA"),       //  1
            B("AAGTGCTAGT"), //  2
            B("ATA"),        //  3
            B("AGTA"),       //  4
            B("GTCCA"),      //  5
            B("GGGT"),       //  6
            B("AACT"),       //  7
            B("AACAT"),      //  8
            B("AGCC"),       //  9
        ];
        /*
        1 ----- 8 --- 4 -----
          \   /   \     \     \
            2      \     \      6
          /   \     \     \   /
        5 ----- 7 --- 3 --- 9
        */

        let _handles = seqs
            .iter()
            .map(|seq| graph.append_handle(seq))
            .collect::<Vec<_>>();

        macro_rules! insert_edges {
            ($graph:ident, [$(($from:literal, $to:literal)),*]) => {
                $(
                    $graph.create_edge(edge($from, $to));
                )*
            };
        }

        insert_edges!(
            graph,
            [
                (1, 2),
                (1, 8),
                (5, 2),
                (5, 7),
                (2, 8),
                (2, 7),
                (7, 3),
                (8, 3),
                (8, 4),
                (3, 9),
                (4, 9),
                (4, 6),
                (9, 6)
            ]
        );
        /* Paths
                path_1: 1 8 4 6
                path_2: 5 2 8 4 6
                path_3: 1 2 8 4 9 6
                path_4: 5 7 3 9 6
        */

        let prep_path =
            |graph: &mut PackedGraph, name: &[u8], steps: Vec<u64>| {
                let path = graph.paths.create_path(name);
                let hnds = vec_hnd(steps);
                (path, hnds)
            };

        let (path_1, p_1_steps) =
            prep_path(&mut graph, b"path1", vec![1, 8, 4, 6]);

        let (path_2, p_2_steps) =
            prep_path(&mut graph, b"path2", vec![5, 2, 8, 4, 6]);

        let (path_3, p_3_steps) =
            prep_path(&mut graph, b"path3", vec![1, 2, 8, 4, 9, 6]);

        let (path_4, p_4_steps) =
            prep_path(&mut graph, b"path4", vec![5, 7, 3, 9, 6]);

        let steps_vecs = vec![p_1_steps, p_2_steps, p_3_steps, p_4_steps];

        graph.zip_all_paths_mut_ctx(
            steps_vecs.into_iter(),
            |steps, _path_id, path| {
                steps
                    .into_iter()
                    .map(|h| path.append_handle(h))
                    .collect::<Vec<_>>()
            },
        );

        let get_occurs = |graph: &PackedGraph, id: u64| {
            let oc_ix = graph.nodes.handle_occur_record(hnd(id)).unwrap();
            let oc_iter = graph.occurrences.iter(oc_ix);
            oc_iter
                .map(|(_occ_ix, record)| {
                    (record.path_id.0, record.offset.pack())
                })
                .collect::<Vec<_>>()
        };

        // let occ_1 = get_occurs(&graph, 1);
        // let occ_2 = get_occurs(&graph, 2);

        // let occ_3 = get_occurs(&graph, 3);
        // let occ_6 = get_occurs(&graph, 6);

        // let occ_7 = get_occurs(&graph, 7);
        // let occ_8 = get_occurs(&graph, 8);

        // remove node 7 from path 4
        graph.with_path_mut_ctx(path_4, |path| {
            if let Some(step) =
                path.remove_step(PathStepIx::from_one_based(2usize))
            {
                vec![step]
            } else {
                Vec::new()
            }
        });

        let occ_7_new = get_occurs(&graph, 7);
        assert!(occ_7_new.is_empty());

        // remove all nodes from path 3
        graph.with_path_mut_ctx(path_3, |path| {
            (0..6)
                .into_iter()
                .filter_map(|i| {
                    path.remove_step(PathStepIx::from_one_based(
                        (i + 1) as usize,
                    ))
                })
                .collect()
        });

        let occ_1_new = get_occurs(&graph, 1);
        let occ_2_new = get_occurs(&graph, 2);
        let occ_6_new = get_occurs(&graph, 6);
        let occ_8_new = get_occurs(&graph, 8);

        assert_eq!(occ_1_new, vec![(0, 1)]);
        assert_eq!(occ_2_new, vec![(1, 2)]);
        assert_eq!(occ_6_new, vec![(3, 5), (1, 5), (0, 4)]);
        assert_eq!(occ_8_new, vec![(1, 3), (0, 2)]);

        let path_steps = |graph: &PackedGraph, id: PathId| {
            let path_ref = graph.paths.path_ref(id).unwrap();

            path_ref
                .steps()
                .map(|(_step_ix, step)| u64::from(step.handle.id()))
                .collect::<Vec<_>>()
        };

        let path_1_steps = path_steps(&graph, path_1);
        let path_2_steps = path_steps(&graph, path_2);
        let path_3_steps = path_steps(&graph, path_3);
        let path_4_steps = path_steps(&graph, path_4);

        assert_eq!(path_1_steps, vec![1, 8, 4, 6]);
        assert_eq!(path_2_steps, vec![5, 2, 8, 4, 6]);
        assert!(path_3_steps.is_empty());
        assert_eq!(path_4_steps, vec![5, 3, 9, 6]);
    }

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

    #[test]
    fn defrag_packed_graph() {
        use bstr::B;

        use paths::tests::{apply_step_ops, StepOp};

        let hnd = |x: u64| Handle::pack(x, false);
        let vec_hnd = |v: Vec<u64>| v.into_iter().map(hnd).collect::<Vec<_>>();
        let edge = |l: u64, r: u64| Edge(hnd(l), hnd(r));

        let mut graph = PackedGraph::new();

        let path_names = [B("path0"), B("path1"), B("path2"), B("path3")];

        let _path_ids = path_names
            .iter()
            .map(|n| graph.paths.create_path(n))
            .collect::<Vec<_>>();

        let seqs = vec![
            //                  Node
            B("GTCA"),       //  1
            B("AAGTGCTAGT"), //  2
            B("ATAGCT"),     //  3
            B("AGTTTTA"),    //  4
            B("GTAATCCA"),   //  5
            B("GGGTGGT"),    //  6
            B("AGCTACT"),    //  7
            B("AACAAAAT"),   //  8
            B("AGCC"),       //  9
        ];
        /*
        1 ----- 8 --- 4 -----
          \   /   \ /   \     \
            2      \     \---- 6
          /   \   / \   / \   /
        5 ----- 7 --- 3 --- 9
        */

        let _handles = seqs
            .iter()
            .map(|seq| graph.append_handle(seq))
            .collect::<Vec<_>>();

        macro_rules! insert_edges {
            ($graph:ident, [$(($from:literal, $to:literal)),*]) => {
                $(
                    $graph.create_edge(edge($from, $to));
                )*
            };
        }

        insert_edges!(
            graph,
            [
                (1, 2),
                (1, 8),
                (5, 2),
                (5, 7),
                (2, 8),
                (2, 7),
                (7, 3),
                (7, 4),
                (8, 3),
                (8, 4),
                (3, 6),
                (3, 9),
                (4, 9),
                (4, 6),
                (9, 6)
            ]
        );

        /* Paths
              path0 - 1  2  7  3  6
              path1 - 5  2  8  4  9  6
              path2 - 1  8  4  6
              path3 - 5  7  3  9  6


        */

        let path_names = [B("path0"), B("path1"), B("path2"), B("path3")];

        let handles_on = |paths: &PackedGraphPaths, id: u64| -> Vec<Handle> {
            let path_ref = paths.path_ref(PathId(id)).unwrap();
            let head = path_ref.properties.head;
            let tail = path_ref.properties.tail;
            let path = path_ref.path;
            paths::packedpath::tests::path_handles(path, head, tail)
        };

        let vectors_for = |paths: &PackedGraphPaths,
                           id: u64|
        // (step_ix, node, prev, next)
         -> Vec<(usize, u64, u64, u64)> {
            let path_ref = paths.path_ref(PathId(id)).unwrap();
            let path = path_ref.path;
            paths::packedpath::tests::path_vectors(path)
        };

        let ops_0 = crate::step_ops![A 5, RE 2, A 1, M 1];
        let ops_1 = crate::step_ops![A 3, RE 1, A 1, RS 1, P 1, A 2, RE 1, M 1, RE 1, A 1];
        let ops_2 = crate::step_ops![A 7, RE 6, A 1];

        graph.with_path_mut_ctx(PathId(0), |ref_mut| {
            apply_step_ops(ref_mut, &ops_0)
        });

        graph.with_path_mut_ctx(PathId(1), |ref_mut| {
            let mut updates = apply_step_ops(ref_mut, &ops_1);
            updates.push(ref_mut.append_handle(hnd(6)));
            updates
        });

        graph.with_path_mut_ctx(PathId(2), |ref_mut| {
            let mut updates = apply_step_ops(ref_mut, &ops_2);
            for h in vec_hnd(vec![4, 6]) {
                updates.push(ref_mut.append_handle(h));
            }
            updates
        });

        graph.with_path_mut_ctx(PathId(3), |ref_mut| {
            let mut updates = Vec::new();
            for h in vec_hnd(vec![5, 7, 3, 9, 6]) {
                updates.push(ref_mut.append_handle(h));
            }
            updates
        });

        let print_path_debug = |graph: &PackedGraph, id: u64| {
            let path_ref = graph.paths.path_ref(PathId(id)).unwrap();
            let head = path_ref.properties.head;
            let tail = path_ref.properties.tail;
            paths::packedpath::tests::print_path(&path_ref.path, head, tail);
            paths::packedpath::tests::print_path_vecs(&path_ref.path);
        };

        println!("path0");
        print_path_debug(&graph, 0);
        println!();

        println!("path1");
        print_path_debug(&graph, 1);
        println!();

        println!("path2");
        print_path_debug(&graph, 2);
        println!();

        println!("path3");
        print_path_debug(&graph, 3);
        println!();

        let get_occurs = |graph: &PackedGraph, id: u64| {
            let oc_ix = graph.nodes.handle_occur_record(hnd(id)).unwrap();
            let oc_iter = graph.occurrences.iter(oc_ix);
            oc_iter
                .map(|(_occ_ix, record)| {
                    (record.path_id.0, record.offset.pack())
                })
                .collect::<Vec<_>>()
        };

        for n in 1..=9 {
            let occ = get_occurs(&graph, n);
            println!("{:2}", n);
            println!("  - {:?}", occ);
            println!();
        }

        /* Occurrences at this point
        1 - [(2, 1), (0, 1)]
        2 - [(1, 2), (0, 2)]
        3 - [(3, 3), (0, 3)]
        4 - [(2, 9), (1, 4)]
        5 - [(3, 1), (1, 5)]
        6 - [(3, 5), (2, 10), (1, 10), (0, 6)]
        7 - [(3, 2), (0, 7)]
        8 - [(2, 8), (1, 8)]
        9 - [(3, 4), (1, 9)]
               */

        graph.defragment();

        println!("---------------------");
        println!(" Defragmenting graph ");
        println!("---------------------");

        println!("path0");
        print_path_debug(&graph, 0);
        println!();

        println!("path1");
        print_path_debug(&graph, 1);
        println!();

        println!("path2");
        print_path_debug(&graph, 2);
        println!();

        println!("path3");
        print_path_debug(&graph, 3);
        println!();

        for n in 1..=9 {
            let occ = get_occurs(&graph, n);
            println!("{:2}", n);
            println!("  - {:?}", occ);
            println!();
        }

        /* After defragmenting, all nodes should have the same
         * occurrences, only with shifted step offsets
         1 - [(2, 1), (0, 1)]
         2 - [(1, 1), (0, 2)]
         3 - [(3, 3), (0, 3)]
         4 - [(2, 3), (1, 2)]
         5 - [(3, 1), (1, 3)]
         6 - [(3, 5), (2, 4), (1, 6), (0, 4)]
         7 - [(3, 2), (0, 5)]
         8 - [(2, 2), (1, 4)]
         9 - [(3, 4), (1, 5)]
        */

        // TODO make sure this will actually change some edge list heads!
        // remove edges (2, 7), (8, 3), (4, 6)
        graph.remove_edge(edge(2, 7));
        graph.remove_edge(edge(8, 3));
        graph.remove_edge(edge(4, 6));

        // Check new edge lists

        // Defragment

        // Compare post-defrag edge lists against pre-defrag

        // Remove nodes 8, 4, 9

        // Check new edge lists
        // Check new node occurrences
        // Check paths

        // Defragment

        // Check new edge lists
        // Check new node occurrences
        // Check paths

        // Check
    }
}
