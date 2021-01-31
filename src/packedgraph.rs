/*!

A handlegraph implementation using [`packed`](crate::packed) vector
representations to minimize memory usage.

*/

#[allow(unused_imports)]
use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{
        AdditiveHandleGraph, MutableHandleGraph, MutableHandles,
        SubtractiveHandleGraph, TransformNodeIds,
    },
    pathhandlegraph::{
        GraphPathNames, GraphPaths, GraphPathsRef, IntoNodeOccurrences,
        IntoPathIds, MutPath, MutableGraphPaths, PathId, PathSequences,
        PathSteps,
    },
};

use rayon::prelude::*;

use self::graph::SeqRecordIx;

pub mod defragment;
pub mod edges;
pub mod graph;
pub mod index;
pub mod iter;
pub mod nodes;
pub mod occurrences;
pub mod paths;
pub mod sequence;

pub use graph::PackedGraph;

use edges::{EdgeListIx, EdgeLists};
use index::{list, OneBasedIndex, RecordIndex};
use iter::EdgeListHandleIter;
use nodes::IndexMapIter;
use occurrences::OccurrencesIter;
use paths::packedpath::StepPtr;
use sequence::DecodeIter;

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use crate::packed::*;

#[cfg(test)]
pub(crate) mod quickcheck;

impl HandleGraph for PackedGraph {
    #[inline]
    fn min_node_id(&self) -> NodeId {
        self.nodes.min_id().into()
    }
    #[inline]
    fn max_node_id(&self) -> NodeId {
        self.nodes.max_id().into()
    }

    #[inline]
    fn node_count(&self) -> usize {
        self.nodes.node_count()
    }

    #[inline]
    fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[inline]
    fn total_length(&self) -> usize {
        self.nodes.sequences().total_length()
    }
}

impl<'a> IntoHandles for &'a PackedGraph {
    type Handles = NodeIdHandles<IndexMapIter<'a>>;

    #[inline]
    fn handles(self) -> Self::Handles {
        let iter = self.nodes.node_ids_iter();
        NodeIdHandles::new(iter)
    }

    #[inline]
    fn has_node<I: Into<NodeId>>(self, n_id: I) -> bool {
        self.nodes.has_node(n_id)
    }
}

impl<'a> IntoHandlesPar for &'a PackedGraph {
    type HandlesPar = rayon::iter::IterBridge<NodeIdHandles<IndexMapIter<'a>>>;

    #[inline]
    fn handles_par(self) -> Self::HandlesPar {
        self.handles().par_bridge()
    }
}

impl<'a> IntoEdges for &'a PackedGraph {
    type Edges = EdgesIter<&'a PackedGraph>;

    #[inline]
    fn edges(self) -> Self::Edges {
        EdgesIter::new(self)
    }
}

impl<'a> IntoEdgesPar for &'a PackedGraph {
    type EdgesPar = rayon::iter::IterBridge<EdgesIter<&'a PackedGraph>>;

    #[inline]
    fn edges_par(self) -> Self::EdgesPar {
        self.edges().par_bridge()
    }
}

impl<'a> IntoNeighbors for &'a PackedGraph {
    type Neighbors = EdgeListHandleIter<'a>;

    #[inline]
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors {
        use Direction as Dir;
        if !self.has_node(handle.id()) {
            panic!(
                "tried to get neighbors of node {} which doesn't exist",
                handle.id().0
            );
        }
        let g_ix = self.nodes.handle_record(handle).unwrap();

        let edge_list_ix = match (dir, handle.is_reverse()) {
            (Dir::Left, true) | (Dir::Right, false) => {
                self.nodes.get_edge_list(g_ix, Direction::Right)
            }
            (Dir::Left, false) | (Dir::Right, true) => {
                self.nodes.get_edge_list(g_ix, Direction::Left)
            }
        };

        let iter = self.edges.iter(edge_list_ix);

        EdgeListHandleIter::new(iter, dir == Dir::Left)
    }
}

impl<'a> IntoSequences for &'a PackedGraph {
    type Sequence = DecodeIter<'a>;

    #[inline]
    fn sequence(self, handle: Handle) -> Self::Sequence {
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

impl<'a> IntoNodeOccurrences for &'a PackedGraph {
    type Occurrences = OccurrencesIter<'a>;
    #[inline]
    fn steps_on_handle(self, handle: Handle) -> Option<Self::Occurrences> {
        let occ_ix = self.nodes.handle_occur_record(handle)?;
        let iter = self.occurrences.iter(occ_ix);
        Some(OccurrencesIter::new(iter))
    }
}

impl AdditiveHandleGraph for PackedGraph {
    #[inline]
    fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = self.max_node_id() + 1;
        self.create_handle(sequence, id)
    }

    #[inline]
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

    #[inline]
    fn create_edge(&mut self, Edge(left, right): Edge) {
        if self.has_edge(left, right) {
            return;
        }

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

        let right_edge_list =
            self.nodes.get_edge_list(right_gix, right_edge_dir);

        // create the record for the edge from the left handle to the right
        let left_to_right = self.edges.append_record(right, left_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the left handle
        self.nodes
            .set_edge_list(left_gix, left_edge_dir, left_to_right);

        // don't add a reversing self-edge twice
        if left == right.flip() {
            // if left_edge_list == right_edge_list {
            self.edges.reversing_self_edge_records += 1;
            return;
        }

        // create the record for the edge from the right handle to the left
        let right_to_left =
            self.edges.append_record(left.flip(), right_edge_list);

        // set the `next` pointer of the new record to the old head of
        // the right handle
        self.nodes
            .set_edge_list(right_gix, right_edge_dir, right_to_left);
    }
}

impl SubtractiveHandleGraph for PackedGraph {
    #[inline]
    fn remove_handle(&mut self, handle: Handle) -> bool {
        self.remove_handle_impl(handle).is_some()
    }

    #[inline]
    fn remove_edge(&mut self, Edge(left, right): Edge) -> bool {
        if !self.has_edge(left, right) {
            return false;
        }

        self.remove_edge_from(left, right);

        if left != right.flip() {
            self.remove_edge_from(right.flip(), left.flip());
        }

        true
    }

    fn clear_graph(&mut self) {
        std::mem::swap(self, &mut PackedGraph::default());
    }
}

impl MutableHandles for PackedGraph {
    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: &[usize],
    ) -> Vec<Handle> {
        let node_len = self.node_len(handle);

        let fwd_handle = handle.forward();

        let mut result = vec![fwd_handle];

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

        // TODO handle self-edges
        // Get the edge lists with the back references
        let right_neighbors = self
            .neighbors(last_handle, Direction::Right)
            .map(|h| {
                let g_ix = self.nodes.handle_record(h).unwrap();
                if h.is_reverse() {
                    self.nodes.get_edge_list(g_ix, Direction::Right)
                } else {
                    self.nodes.get_edge_list(g_ix, Direction::Left)
                }
            })
            .collect::<Vec<_>>();

        // Update the corresponding edge record in each of the
        // neighbor back reference lists
        for edge_list in right_neighbors {
            self.edges.update_edge_record(
                edge_list,
                |_, (h, _)| h.flip() == handle,
                |(_, n)| (last_handle.flip(), n),
            );
        }

        // create edges between the new segments
        for window in result.windows(2) {
            if let [this, next] = *window {
                self.create_edge(Edge(this, next));
            }
        }

        let occurrences =
            self.steps_on_handle(handle).unwrap().collect::<Vec<_>>();

        for (path_id, step_ix) in occurrences {
            self.with_path_mut_ctx(path_id, |path_mut| {
                let last_step = step_ix;
                result
                    .iter()
                    .skip(1)
                    .filter_map(|&h| path_mut.insert_step_after(last_step, h))
                    .collect()
            });
        }

        result
    }

    fn apply_orientation(&mut self, handle: Handle) -> Handle {
        if !handle.is_reverse() {
            return handle;
        }

        let g_ix = self.nodes.handle_record(handle).unwrap();

        // Overwrite the sequence with its reverse complement
        let rev_seq = self.sequence_vec(handle);
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

        let occurrences =
            self.steps_on_handle(handle).unwrap().collect::<Vec<_>>();

        for (path_id, step_ix) in occurrences {
            self.with_path_mut_ctx(path_id, |path_mut| {
                path_mut
                    .flip_step(step_ix)
                    .unwrap_or_default()
                    .into_iter()
                    .collect()
            });
        }

        handle.flip()
    }
}

impl TransformNodeIds for PackedGraph {
    fn transform_node_ids<F>(&mut self, transform: F)
    where
        F: Fn(NodeId) -> NodeId + Copy + Send + Sync,
    {
        // Update the targets of all edges
        let length = self.edges.record_count();

        for ix in 0..length {
            let tgt_ix = 2 * ix;
            let handle: Handle = self.edges.record_vec.get_unpack(tgt_ix);
            let n_id = handle.id();

            if !n_id.is_zero() && self.has_node(n_id) {
                let new_handle =
                    Handle::pack(transform(n_id), handle.is_reverse());
                self.edges.record_vec.set_pack(tgt_ix, new_handle);
            }
        }

        // Create a new NodeIdIndexMap
        self.nodes.transform_node_ids(transform);

        // Update the steps of all paths
        self.with_all_paths_mut_ctx(|_, path_ref| {
            path_ref.path.transform_steps(transform);
            Vec::new()
        });
    }

    fn transform_node_ids_mut<F>(&mut self, mut transform: F)
    where
        F: FnMut(NodeId) -> NodeId,
    {
        // Update the targets of all edges
        let length = self.edges.record_count();

        for ix in 0..length {
            let tgt_ix = 2 * ix;
            let handle: Handle = self.edges.record_vec.get_unpack(tgt_ix);
            let n_id = handle.id();
            // if !self.has_node(n_id) {
            //     self.edges.record_vec.set(tgt_ix, 0);
            // }
            if !n_id.is_zero() {
                let new_handle =
                    Handle::pack(transform(n_id), handle.is_reverse());
                self.edges.record_vec.set_pack(tgt_ix, new_handle);
            }
        }

        // Create a new NodeIdIndexMap
        self.nodes.transform_node_ids(&mut transform);

        // Update the steps of all paths
        for path in self.paths.paths.iter_mut() {
            path.transform_steps(&mut transform);
        }
    }

    fn apply_ordering(&mut self, order: &[Handle]) {
        assert!(order.len() == self.node_count());

        let rank: fnv::FnvHashMap<NodeId, usize> = self
            .handles()
            .enumerate()
            .map(|(ix, h)| (h.id(), ix))
            .collect();

        PackedGraph::transform_node_ids(self, |node| {
            if let Some(ix) = rank.get(&node) {
                let handle = order[*ix];
                handle.id()
            } else {
                panic!(
                    "error when transforming node {:?}; didn't exist in graph",
                    node
                );
            }
        });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use rayon::prelude::*;

    use super::index::list::*;
    use super::*;

    use crate::packed::PackedElement;

    fn hnd(x: u64) -> Handle {
        Handle::pack(x, false)
    }

    fn r_hnd(x: u64) -> Handle {
        Handle::pack(x, true)
    }

    fn vec_hnd(v: Vec<u64>) -> Vec<Handle> {
        v.into_iter().map(hnd).collect::<Vec<_>>()
    }

    fn edge(l: u64, r: u64) -> Edge {
        Edge(hnd(l), hnd(r))
    }
    fn r_edge(l: u64, r: u64) -> Edge {
        Edge(r_hnd(l), r_hnd(r))
    }

    fn path_steps(graph: &PackedGraph, id: PathId) -> Vec<u64> {
        let path_ref = graph.paths.path_ref(id).unwrap();

        path_ref
            .steps()
            .map(|(_step_ix, step)| u64::from(step.handle.id()))
            .collect::<Vec<_>>()
    }

    fn path_steps_hnd(graph: &PackedGraph, id: PathId) -> Vec<u64> {
        let path_ref = graph.paths.path_ref(id).unwrap();

        path_ref
            .steps()
            .map(|(_step_ix, step)| step.handle.0)
            .collect::<Vec<_>>()
    }

    // fn printpath_detail(graph: &PackedGraph,

    #[allow(dead_code)]
    pub(crate) fn print_path_debug(graph: &PackedGraph, id: u64) {
        let path_ref = graph.paths.path_ref(PathId(id)).unwrap();
        let head = path_ref.head;
        let tail = path_ref.tail;
        paths::packedpath::tests::print_path(&path_ref.path, head, tail);
        paths::packedpath::tests::print_path_vecs(&path_ref.path);
    }

    #[allow(dead_code)]
    pub(crate) fn print_path_data(graph: &PackedGraph, id: PathId) {
        use crate::packed::PackedCollection;
        use bstr::{ByteSlice, ByteVec};
        use paths::AsStepsRef;

        let path_ref = graph.paths.path_ref(id).unwrap();
        let head = path_ref.head;
        let tail = path_ref.tail;

        let name = graph.get_path_name_vec(id).unwrap();

        println!();
        println!("-----------------------------");
        println!(
            "Path \"{}\" - Head {} - Tail {}",
            name.as_bstr(),
            head.pack(),
            tail.pack()
        );

        println!("{:^14}----------", "Iter");
        // print the steps on the path
        let mut ptrs: Vec<u64> = Vec::new();
        let mut handles: Vec<u64> = Vec::new();
        let mut prevs: Vec<u64> = Vec::new();
        let mut nexts: Vec<u64> = Vec::new();

        for (ptr, step) in path_ref.path.iter(head, tail) {
            ptrs.push(ptr.pack());
            handles.push(step.handle.pack());
            prevs.push(step.prev.pack());
            nexts.push(step.next.pack());
        }

        let print_slice = |data: &[u64]| {
            for (ix, v) in data.iter().enumerate() {
                if ix != 0 {
                    print!(", ");
                }
                print!("{:2}", v);
            }
            println!();
        };

        print!(" Index  - ");
        print_slice(&ptrs);
        print!(" Handle - ");
        print_slice(&handles);
        print!(" Prev   - ");
        print_slice(&prevs);
        print!(" Next   - ");
        print_slice(&nexts);

        println!();
        println!("{:^14}----------", "Vectors");

        // print the storage vectors
        ptrs.clear();
        handles.clear();
        prevs.clear();
        nexts.clear();

        let step_record_len = path_ref.path.steps.len();

        for ix in 0..step_record_len {
            ptrs.push((ix + 1) as u64);

            handles.push(path_ref.path.steps.get(ix));
            let l_ix = ix * 2;
            prevs.push(path_ref.path.links.get(l_ix));
            nexts.push(path_ref.path.links.get(l_ix + 1));
        }

        print!(" Index  - ");
        print_slice(&ptrs);
        print!(" Handle - ");
        print_slice(&handles);
        print!(" Prev   - ");
        print_slice(&prevs);
        print!(" Next   - ");
        print_slice(&nexts);

        println!();
        println!("-----------------------------");
    }

    #[allow(dead_code)]
    pub(crate) fn print_node_records(graph: &PackedGraph, ids: &[u64]) {
        println!("{:4}  {:6}  {:5}  {:5}", "Node", "Record", "Left", "Right");
        for &id in ids.iter() {
            let (record, left, right) =
                if let Some(rec_id) = graph.nodes.handle_record(hnd(id)) {
                    let rec = rec_id.pack().to_string();
                    let (left, right) =
                        graph.nodes.get_node_edge_lists(rec_id).unwrap();
                    (rec, left.pack().to_string(), right.pack().to_string())
                } else {
                    (0u64.to_string(), "-".to_string(), "-".to_string())
                };

            println!("{:4}  {:6}  {:5}  {:5}", id, record, left, right);
        }
        println!();
    }

    #[allow(dead_code)]
    pub(crate) fn print_edge_records(graph: &PackedGraph) {
        println!("{:6}  {:6}  {:6}", "EdgeIx", "Target", "Next");

        for ix in 0..graph.edges.record_count() {
            let edge_ix = EdgeListIx::from_zero_based(ix);
            let (target, next) =
                if let Some(record) = graph.edges.get_record(edge_ix) {
                    (record.0.pack().to_string(), record.1.pack().to_string())
                } else {
                    ("-".to_string(), "-".to_string())
                };
            println!("{:6}  {:6}  {:6}", edge_ix.pack(), target, next,);
        }

        println!();
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

    fn get_all_neighbors(
        graph: &PackedGraph,
        handles: &[Handle],
        dir: Direction,
    ) -> Vec<(u64, Vec<u64>)> {
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
    }

    pub(crate) fn test_graph_no_paths() -> PackedGraph {
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

        graph
    }

    pub(crate) fn test_graph_with_paths() -> PackedGraph {
        let mut graph = test_graph_no_paths();
        /* Paths
                path_1: 1 8 4 6
                path_2: 5 2 8 4 6
                path_3: 1 2 8 4 9 6
                path_4: 5 7 3 9 6
        */

        let prep_path =
            |graph: &mut PackedGraph, name: &[u8], steps: Vec<u64>| {
                let path = graph.paths.create_path(name, false);
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
            steps_vecs.into_par_iter(),
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

        let _path_ids = graph.path_ids().collect::<Vec<_>>();

        let path_1 = graph.get_path_id(b"path1").unwrap();
        let path_4 = graph.get_path_id(b"path4").unwrap();

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

        let path_2 = graph.get_path_id(b"path2");
        let path_3 = graph.get_path_id(b"path3");

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

        let path_3 = graph.get_path_id(b"path3").unwrap();
        let path_4 = graph.get_path_id(b"path4").unwrap();

        graph.destroy_path(path_4);

        let node_7_occ_1 = get_occurs(&graph, 7);
        let node_8_occ_1 = get_occurs(&graph, 8);

        assert!(node_7_occ_1.is_empty());
        assert_eq!(node_8_occ, node_8_occ_1);

        graph.destroy_path(path_3);

        let node_8_occ_2 = get_occurs(&graph, 8);

        assert_eq!(node_8_occ_2, vec![(1, 3), (0, 2)]);
    }

    #[test]
    fn append_steps_iter() {
        let mut graph = test_graph_no_paths();

        let handles =
            (1..=9).map(|x| Handle::pack(x, false)).collect::<Vec<_>>();

        let path_0 = graph.paths.create_path(b"path_0", false);
        let path_1 = graph.paths.create_path(b"path_1", false);

        graph.with_path_mut_ctx(path_0, |path_ref| {
            path_ref.append_steps_iter(handles.iter().copied())
        });

        graph.with_path_mut_ctx(path_1, |path_ref| {
            handles
                .iter()
                .copied()
                .map(|h| path_ref.append_step(h))
                .collect::<Vec<_>>()
        });

        let path_0_steps = graph
            .get_path_ref(path_0)
            .unwrap()
            .steps()
            .collect::<Vec<_>>();

        let path_1_steps = graph
            .get_path_ref(path_1)
            .unwrap()
            .steps()
            .collect::<Vec<_>>();

        assert_eq!(path_0_steps, path_1_steps);

        let p0_first = graph.path_first_step(path_0);
        let p0_last = graph.path_last_step(path_0);
        let p1_first = graph.path_first_step(path_1);
        let p1_last = graph.path_last_step(path_1);

        print_path_data(&graph, path_0);
        print_path_data(&graph, path_1);

        println!(
            "path 0 - head: {:?}\t tail: {:?}",
            graph.path_first_step(path_0),
            graph.path_last_step(path_0)
        );
        println!(
            "path 1 - head: {:?}\t tail: {:?}",
            graph.path_first_step(path_1),
            graph.path_last_step(path_1)
        );

        println!("path 0 len: {:?}", graph.path_len(path_0));
        println!("path 1 len: {:?}", graph.path_len(path_1));

        println!(
            "path 0 first: {:#?}",
            graph.path_handle_at_step(path_0, p0_first.unwrap())
        );
        println!(
            "path 0 last:  {:#?}",
            graph.path_handle_at_step(path_0, p0_last.unwrap())
        );

        println!(
            "path 1 first: {:#?}",
            graph.path_handle_at_step(path_1, p1_first.unwrap())
        );
        println!(
            "path 1 last:  {:#?}",
            graph.path_handle_at_step(path_1, p1_last.unwrap())
        );
    }

    #[test]
    fn packedgraph_mutate_paths() {
        let mut graph = test_graph_with_paths();

        let path_1 = PathId(0);
        let path_2 = PathId(1);
        let path_3 = PathId(2);
        let path_4 = PathId(3);

        // remove node 7 from path 4
        graph.with_path_mut_ctx(path_4, |path| {
            if let Some(step) =
                path.remove_step(StepPtr::from_one_based(2usize))
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
                    path.remove_step(StepPtr::from_one_based((i + 1) as usize))
                })
                .collect()
        });

        let expected_occurs = vec![
            vec![(0, 1)],
            vec![(1, 2)],
            vec![(3, 5), (1, 5), (0, 4)],
            vec![(1, 3), (0, 2)],
        ];

        [1, 2, 6, 8]
            .iter()
            .zip(expected_occurs.into_iter())
            .for_each(|(node, expected)| {
                assert_eq!(get_occurs(&graph, *node), expected);
            });

        let expected_steps = vec![
            vec![1, 8, 4, 6],
            vec![5, 2, 8, 4, 6],
            Vec::new(),
            vec![5, 3, 9, 6],
        ];

        [path_1, path_2, path_3, path_4]
            .iter()
            .zip(expected_steps.into_iter())
            .for_each(|(path, expected)| {
                assert_eq!(path_steps(&graph, *path), expected);
            });
    }

    #[test]
    fn divide_handle() {
        use bstr::{BString, B};

        let bseq = |g: &PackedGraph, x: u64| -> BString {
            g.sequence_vec(hnd(x)).into()
        };

        let mut graph = test_graph_with_paths();

        let pre_divide_occurrences =
            (1..=9).map(|n| get_occurs(&graph, n)).collect::<Vec<_>>();

        let new_hs = graph.divide_handle(hnd(2), &[3, 7, 9]);

        assert_eq!(graph.node_count(), 12);

        let post_divide_occurrences =
            (1..=12).map(|n| get_occurs(&graph, n)).collect::<Vec<_>>();

        assert_eq!(&pre_divide_occurrences[..], &post_divide_occurrences[0..9]);

        assert_eq!(
            &post_divide_occurrences[9..],
            &[[(1, 6), (2, 7)], [(1, 7), (2, 8)], [(1, 8), (2, 9)]]
        );

        assert_eq!(bseq(&graph, 2), B("AAG"));

        let new_seqs: Vec<BString> = new_hs
            .iter()
            .map(|h| graph.sequence_vec(*h).into())
            .collect();

        // The sequence is correctly split
        assert_eq!(new_seqs, vec![B("AAG"), B("TGCT"), B("AG"), B("T")]);

        let mut edges = graph.edges().collect::<Vec<_>>();
        edges.sort();

        assert_eq!(
            edges,
            vec![
                edge(1, 2),
                edge(1, 8),
                edge(2, 10),
                r_edge(2, 5),
                edge(3, 9),
                r_edge(3, 7),
                r_edge(3, 8),
                edge(4, 6),
                edge(4, 9),
                r_edge(4, 8),
                edge(5, 7),
                r_edge(6, 9),
                r_edge(7, 12),
                r_edge(8, 12),
                edge(10, 11),
                edge(11, 12),
            ]
        );
    }

    #[test]
    fn defrag_packed_graph() {
        use bstr::B;
        use defragment::Defragment;
        use paths::tests::{apply_step_ops, StepOp};

        let get_neighbors = |graph: &PackedGraph, x: u64| {
            if let Some(_rec_id) = graph.nodes.handle_record(hnd(x)) {
                let left = graph
                    .neighbors(hnd(x), Direction::Left)
                    .map(|h| u64::from(h.id()))
                    .collect::<Vec<_>>();
                let right = graph
                    .neighbors(hnd(x), Direction::Right)
                    .map(|h| u64::from(h.id()))
                    .collect::<Vec<_>>();
                (left, right)
            } else {
                (Vec::new(), Vec::new())
            }
        };

        let mut graph = test_graph_no_paths();

        graph.create_edge(edge(7, 4));
        graph.create_edge(edge(3, 6));

        let path_names = [B("path0"), B("path1"), B("path2"), B("path3")];

        let _path_ids = path_names
            .iter()
            .map(|n| graph.paths.create_path(n, false))
            .collect::<Vec<_>>();

        /* Paths
              path0 - 1  2  7  3  6
              path1 - 5  2  8  4  9  6
              path2 - 1  8  4  6
              path3 - 5  7  3  9  6
        */

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

        let post_defrag_occurrences =
            (1..=9).map(|n| get_occurs(&graph, n)).collect::<Vec<_>>();

        /* After defragmenting, all nodes should have the same
         * occurrences, only with shifted step offsets
         */

        assert_eq!(
            post_defrag_occurrences,
            vec![
                vec![(2, 1), (0, 1)],
                vec![(1, 1), (0, 2)],
                vec![(3, 3), (0, 3)],
                vec![(2, 3), (1, 2)],
                vec![(3, 1), (1, 3)],
                vec![(3, 5), (2, 4), (1, 6), (0, 4)],
                vec![(3, 2), (0, 5)],
                vec![(2, 2), (1, 4)],
                vec![(3, 4), (1, 5)]
            ]
        );

        // remove edges (2, 7), (8, 3), (4, 6),
        // corresponding to indices 11, 12, 17, 18, 27, 28
        graph.remove_edge(edge(2, 7));
        graph.remove_edge(edge(8, 3));
        graph.remove_edge(edge(4, 6));

        // Check new edge lists

        let pre_defrag_neighbors = (1..=9)
            .map(|n| get_neighbors(&graph, n))
            .collect::<Vec<_>>();

        // Defragment
        graph.defragment();

        let post_defrag_neighbors = (1..=9)
            .map(|n| get_neighbors(&graph, n))
            .collect::<Vec<_>>();

        // Neighbors should not be affected by defragmentation
        assert_eq!(pre_defrag_neighbors, post_defrag_neighbors);

        // Remove node 4
        graph.remove_handle(hnd(4));

        let mut pre_defrag_neighbors = (1..=9)
            .map(|n| get_neighbors(&graph, n))
            .collect::<Vec<_>>();

        graph.defragment();

        let post_defrag_neighbors = [1, 2, 3, 5, 6, 7, 8, 9]
            .iter()
            .map(|n| get_neighbors(&graph, *n))
            .collect::<Vec<_>>();

        // Other than the removed node, the neighbor lists should be
        // unaffected
        pre_defrag_neighbors.remove(3);

        assert_eq!(pre_defrag_neighbors, post_defrag_neighbors);

        let post_defrag_occurrences = [1, 2, 3, 5, 6, 7, 8, 9]
            .iter()
            .map(|n| get_occurs(&graph, *n))
            .collect::<Vec<_>>();

        assert_eq!(
            post_defrag_occurrences,
            vec![
                vec![(0, 1)],
                vec![(0, 2)],
                vec![(1, 3), (0, 3)],
                vec![(1, 1)],
                vec![(1, 5), (0, 4)],
                vec![(1, 2), (0, 5)],
                vec![],
                vec![(1, 4)]
            ]
        );

        // The path_1 and path_2 both included the removed node, so
        // they have been deleted
        let expected_path_ids =
            vec![Some(PathId(0)), None, None, Some(PathId(1))];

        path_names
            .iter()
            .zip(expected_path_ids.into_iter())
            .for_each(|(name, expected)| {
                assert_eq!(graph.get_path_id(name), expected);
            });
    }

    #[test]
    fn path_rewrite_segment() {
        use bstr::B;

        let mut graph = test_graph_with_paths();
        // add some extra sequences
        graph.append_handle(b"GGTCGTCTGG");
        graph.append_handle(b"ATGT");
        graph.append_handle(b"AAATGA");
        graph.append_handle(b"TTTGTGTA");

        let graph_2 = graph.clone();

        let path_names = vec![B("path1"), B("path2"), B("path3"), B("path4")];
        let path_ids = path_names
            .iter()
            .filter_map(|&n| graph.get_path_id(n))
            .collect::<Vec<_>>();

        /* Paths before
          steps path_1: 1  2  3  4
          nodes path_1: 1+ 8+ 4+ 6+

          steps path_2: 1  2  3  4  5
          nodes path_2: 5+ 2+ 8+ 4+ 6+

          steps path_3: 1  2  3  4  5  6
          nodes path_3: 1+ 2+ 8+ 4+ 9+ 6+

          steps path_4: 1  2  3  4  5
          nodes path_4: 5+ 7+ 3+ 9+ 6+
        */

        let print_paths = |graph: &PackedGraph, ids: &[PathId]| {
            for &id in ids {
                print_path_data(graph, id);
                println!();
            }
        };

        print_paths(&graph, &path_ids);
        println!("  ---  Deleting  segments  ---");

        let step_ptr =
            |ix: u64| -> StepPtr { StepPtr::from_one_based(ix as usize) };

        let step_range = |l: u64, r: u64| -> (StepPtr, StepPtr) {
            (
                StepPtr::from_one_based(l as usize),
                StepPtr::from_one_based(r as usize),
            )
        };

        /* Remove ranges:

            path_1: [1, 3)    - []
            path_2: [2, 4)    - []
            path_3: [4, null) - []
            path_4: [1, null) - []
        */
        // let
        let segment_ranges = vec![
            step_range(1, 3),
            step_range(2, 4),
            (step_ptr(4), StepPtr::null()),
            (step_ptr(1), StepPtr::null()),
        ];

        for (ix, &(from, to)) in segment_ranges.iter().enumerate() {
            graph.path_rewrite_segment(path_ids[ix], from, to, &[]);
        }

        print_paths(&graph, &path_ids);

        assert_eq!(path_steps_hnd(&graph, path_ids[0]), vec![8, 12]);
        assert_eq!(path_steps_hnd(&graph, path_ids[1]), vec![10, 8, 12]);
        assert_eq!(path_steps_hnd(&graph, path_ids[2]), vec![2, 4, 16]);
        assert_eq!(path_steps_hnd(&graph, path_ids[3]), vec![]);

        // make sure that a single-step range is treated as empty
        graph.path_rewrite_segment(path_ids[1], step_ptr(2), step_ptr(2), &[]);
        assert_eq!(path_steps_hnd(&graph, path_ids[1]), vec![10, 8, 12]);

        /* Rewrite paths with:

            path_1: [1, 3)    - [5+, 2+]
            path_2: [2, 4)    - [7+, 3-, 10+, 12-]
            path_3: [4, null) - [6+, 3+, 5-]
            path_4: [1, null) - [3+, 6+, 2+, 4+]
        */

        println!("  ---  Rewriting segments  ---");

        let mut graph = graph_2;

        let new_segments = vec![
            vec![hnd(5), hnd(2)],
            vec![hnd(7), r_hnd(3), hnd(10), r_hnd(12)],
            vec![hnd(6), hnd(3), r_hnd(5)],
            vec_hnd(vec![3, 6, 2, 4]),
        ];

        for (ix, &(from, to)) in segment_ranges.iter().enumerate() {
            graph.path_rewrite_segment(
                path_ids[ix],
                from,
                to,
                &new_segments[ix],
            );
        }

        assert_eq!(path_steps_hnd(&graph, path_ids[0]), vec![10, 4, 8, 12]);
        assert_eq!(
            path_steps_hnd(&graph, path_ids[1]),
            vec![10, 14, 7, 20, 25, 8, 12]
        );
        assert_eq!(
            path_steps_hnd(&graph, path_ids[2]),
            vec![2, 4, 16, 12, 6, 11]
        );
        assert_eq!(path_steps_hnd(&graph, path_ids[3]), vec![6, 12, 4, 8]);

        print_path_data(&graph, path_ids[1]);

        println!("-------------");

        // make sure that a single-step range is treated as empty, and
        // the new range prepended
        graph.path_rewrite_segment(
            path_ids[1],
            step_ptr(7),
            step_ptr(7),
            &vec_hnd(vec![1, 2, 3]),
        );

        assert_eq!(
            path_steps_hnd(&graph, path_ids[1]),
            vec![10, 14, 2, 4, 6, 7, 20, 25, 8, 12]
        );
        print_path_data(&graph, path_ids[1]);
    }

    #[test]
    fn reassign_node_ids() {
        let mut graph = test_graph_with_paths();

        use fnv::FnvHashMap;

        let slice_hnd =
            |slice: &[u64]| slice.iter().map(|n| hnd(*n)).collect::<Vec<_>>();

        let node_ids: Vec<u64> = (1u64..=9).collect();

        let transformed_ids: Vec<u64> =
            vec![43, 132, 5, 872, 273, 111, 987, 8, 9839];

        let id_map = node_ids
            .iter()
            .copied()
            .zip(transformed_ids.iter().copied())
            .collect::<FnvHashMap<u64, u64>>();

        let transform = |n: NodeId| -> NodeId {
            NodeId::from(id_map.get(&u64::from(n)).copied().unwrap_or(0))
        };

        let get_neighbors = |graph: &PackedGraph, ids: &[u64]| {
            let handles = slice_hnd(ids);
            let left = get_all_neighbors(&graph, &handles, Direction::Left);
            let right = get_all_neighbors(&graph, &handles, Direction::Right);
            left.into_iter()
                .zip(right.into_iter())
                .map(|((n, left), (_n, right))| (left, n, right))
                .collect::<Vec<_>>()
        };

        let pre_occurs = node_ids
            .iter()
            .map(|id| get_occurs(&graph, *id))
            .collect::<Vec<_>>();

        graph.transform_node_ids(transform);

        // Node neighbors, and thus the edge lists, are transformed correctly
        let post_transform_neighbors = get_neighbors(&graph, &transformed_ids);
        assert_eq!(
            post_transform_neighbors,
            vec![
                (vec![], 43, vec![8, 132]),
                (vec![273, 43], 132, vec![987, 8]),
                (vec![8, 987], 5, vec![9839]),
                (vec![8], 872, vec![111, 9839]),
                (vec![], 273, vec![987, 132]),
                (vec![9839, 872], 111, vec![]),
                (vec![132, 273], 987, vec![5]),
                (vec![132, 43], 8, vec![872, 5]),
                (vec![872, 5], 9839, vec![111])
            ]
        );

        // The steps on the paths are the same modulo the ID transformation
        let post_steps = (0..=3)
            .map(|id| path_steps(&graph, PathId(id)))
            .collect::<Vec<_>>();

        assert_eq!(
            post_steps,
            vec![
                vec![43, 8, 872, 111],
                vec![273, 132, 8, 872, 111],
                vec![43, 132, 8, 872, 9839, 111],
                vec![273, 987, 5, 9839, 111]
            ]
        );

        // The node occurrences are identical as the paths have not changed
        let post_occurs = transformed_ids
            .iter()
            .map(|id| get_occurs(&graph, *id))
            .collect::<Vec<_>>();

        assert_eq!(pre_occurs, post_occurs);
    }
}
