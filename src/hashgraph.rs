/*!

A handlegraph implementation using `HashMap` to represent the graph
topology and nodes, and each path as a `Vec` of nodes.
*/

use rayon::prelude::*;

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::{
        GraphPathNames, GraphPaths, GraphPathsRef, IntoNodeOccurrences,
        IntoPathIds, MutableGraphPaths, PathId, PathSequences,
    },
    util::dna,
};

mod graph;
pub mod node;
pub mod path;

pub use self::graph::*;
pub use self::node::Node;
pub use self::path::Path;

impl HandleGraph for HashGraph {
    #[inline]
    fn min_node_id(&self) -> NodeId {
        self.min_id
    }

    #[inline]
    fn max_node_id(&self) -> NodeId {
        self.max_id
    }

    #[inline]
    fn node_count(&self) -> usize {
        self.graph.len()
    }

    #[inline]
    fn edge_count(&self) -> usize {
        self.edges().count()
    }

    fn total_length(&self) -> usize {
        self.handles().map(|h| self.node_len(h)).sum()
    }
}

impl<'a> IntoHandles for &'a HashGraph {
    type Handles = NodeIdRefHandles<
        'a,
        std::collections::hash_map::Keys<'a, NodeId, Node>,
    >;

    #[inline]
    fn handles(self) -> Self::Handles {
        let keys = self.graph.keys();
        NodeIdRefHandles::new(keys)
    }

    #[inline]
    fn has_node<I: Into<NodeId>>(self, n_id: I) -> bool {
        self.graph.contains_key(&n_id.into())
    }
}

// While this is a valid implementation of `IntoHandles`, it's much
// slower as the compiler can't inline the closure.
/*
impl<'a> IntoHandles for &'a HashGraph {
    type Handles = std::iter::Map<
        std::collections::hash_map::Keys<'a, NodeId, Node>,
        fn(&'a NodeId) -> Handle,
    >;

    #[inline]
    fn handles(self) -> Self::Handles {
        let keys = self.graph.keys();
        keys.map(|&n_id| Handle::pack(n_id, false))
    }
}
*/

impl<'a> IntoHandlesPar for &'a HashGraph {
    type HandlesPar = rayon::iter::IterBridge<
        NodeIdRefHandles<
            'a,
            std::collections::hash_map::Keys<'a, NodeId, Node>,
        >,
    >;

    fn handles_par(self) -> Self::HandlesPar {
        self.handles().par_bridge()
    }
}

impl<'a> IntoEdges for &'a HashGraph {
    type Edges = EdgesIter<&'a HashGraph>;

    #[inline]
    fn edges(self) -> Self::Edges {
        EdgesIter::new(self)
    }
}

impl<'a> IntoNeighbors for &'a HashGraph {
    type Neighbors = NeighborIter<'a, std::slice::Iter<'a, Handle>>;

    #[inline]
    fn neighbors(self, handle: Handle, dir: Direction) -> Self::Neighbors {
        let node = self.get_node_unchecked(&handle.id());

        let handles = match (dir, handle.is_reverse()) {
            (Direction::Left, true) => &node.right_edges,
            (Direction::Left, false) => &node.left_edges,
            (Direction::Right, true) => &node.left_edges,
            (Direction::Right, false) => &node.right_edges,
        };

        NeighborIter::new(handles.iter(), dir == Direction::Left)
    }

    #[inline]
    fn degree(self, handle: Handle, dir: Direction) -> usize {
        let n = self.get_node_unchecked(&handle.id());
        match dir {
            Direction::Right => n.right_edges.len(),
            Direction::Left => n.left_edges.len(),
        }
    }
}

impl<'a> IntoSequences for &'a HashGraph {
    type Sequence = SequenceIter<std::iter::Copied<std::slice::Iter<'a, u8>>>;

    #[inline]
    fn sequence(self, handle: Handle) -> Self::Sequence {
        let seq: &[u8] =
            &self.get_node_unchecked(&handle.id()).sequence.as_ref();
        SequenceIter::new(seq.iter().copied(), handle.is_reverse())
    }

    fn sequence_vec(self, handle: Handle) -> Vec<u8> {
        let seq: &[u8] =
            &self.get_node_unchecked(&handle.id()).sequence.as_ref();
        if handle.is_reverse() {
            dna::rev_comp(seq)
        } else {
            seq.into()
        }
    }

    #[inline]
    fn node_len(self, handle: Handle) -> usize {
        self.get_node_unchecked(&handle.id()).sequence.len()
    }
}

impl AdditiveHandleGraph for HashGraph {
    fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        self.create_handle(sequence, self.max_id + 1)
    }

    fn create_handle<T: Into<NodeId>>(
        &mut self,
        seq: &[u8],
        node_id: T,
    ) -> Handle {
        let id: NodeId = node_id.into();

        if seq.is_empty() {
            panic!("Tried to add empty handle");
        }
        self.graph.insert(id, Node::new(seq));
        self.max_id = std::cmp::max(self.max_id, id);
        self.min_id = std::cmp::min(self.min_id, id);
        Handle::pack(id, false)
    }

    fn create_edge(&mut self, Edge(left, right): Edge) {
        /*
        let add_edge = {
            if left != right.flip() {
                let left_node = self
                    .graph
                    .get(&left.id())
                    .expect("Node doesn't exist for the given handle");

                None == left_node
                    .right_edges
                    .iter()
                    .find(|&&h| h == right.flip())
            } else {
                let left_node = self
                    .graph
                    .get(&left.id())
                    .expect("Node doesn't exist for the given handle");

                None == left_node.right_edges.iter().find(|&&h| h == right)
            }
        };
        */

        let left_node = self
            .graph
            .get_mut(&left.id())
            .expect("Node doesn't exist for the given handle");
        if left.is_reverse() {
            if !left_node.left_edges.contains(&right) {
                left_node.left_edges.push(right);
            }
        } else {
            if !left_node.right_edges.contains(&right) {
                left_node.right_edges.push(right);
            }
        }
        if left != right.flip() {
            let right_node = self
                .graph
                .get_mut(&right.id())
                .expect("Node doesn't exist for the given handle");
            if right.is_reverse() {
                if !right_node.right_edges.contains(&left.flip()) {
                    right_node.right_edges.push(left.flip());
                }
            } else {
                if !right_node.left_edges.contains(&left.flip()) {
                    right_node.left_edges.push(left.flip());
                }
            }
        }
    }
}

impl MutableHandles for HashGraph {
    fn divide_handle(
        &mut self,
        handle: Handle,
        offsets: &[usize],
    ) -> Vec<Handle> {
        let mut result = vec![handle];
        let node_len = self.node_len(handle);
        let sequence = self.sequence_vec(handle);

        let fwd_handle = handle.forward();

        let mut offsets = offsets.to_vec();

        // Push the node length as a last offset to make constructing
        // the ranges nicer
        offsets.push(node_len);

        let fwd_offsets: Vec<usize> = if handle.is_reverse() {
            offsets.iter().map(|o| node_len - o).collect()
        } else {
            offsets
        };

        // staggered zip of the offsets with themselves to make the ranges
        let ranges: Vec<_> = fwd_offsets
            .iter()
            .zip(fwd_offsets.iter().skip(1))
            .map(|(&p, &n)| p..n)
            .collect();

        // TODO it should be possible to do this without creating new
        // strings and collecting into a vec
        let subseqs: Vec<Vec<u8>> =
            ranges.into_iter().map(|r| sequence[r].to_owned()).collect();

        for seq in subseqs {
            let h = self.append_handle(&seq);
            result.push(h);
        }

        // move the outgoing edges to the last new segment
        // empty the existing right edges of the original node
        let mut orig_rights = std::mem::take(
            &mut self.get_node_mut(&handle.id()).unwrap().right_edges,
        );

        let new_rights = &mut self
            .get_node_mut(&result.last().unwrap().id())
            .unwrap()
            .right_edges;
        // and swap with the new right edges
        std::mem::swap(&mut orig_rights, new_rights);

        // shrink the sequence of the starting handle
        let orig_node = &mut self.get_node_mut(&handle.id()).unwrap();
        orig_node.sequence = orig_node.sequence[0..fwd_offsets[0]].into();

        // update backwards references
        // first collect all the handles whose nodes we need to update
        let last_neighbors: Vec<_> = self
            .neighbors(*result.last().unwrap(), Direction::Right)
            .collect();

        // And perform the update
        for h in last_neighbors {
            let node = &mut self.get_node_mut(&h.id()).unwrap();
            let neighbors = if h.is_reverse() {
                &mut node.right_edges
            } else {
                &mut node.left_edges
            };

            for bwd in neighbors.iter_mut() {
                if *bwd == fwd_handle.flip() {
                    *bwd = result.last().unwrap().flip();
                }
            }
        }

        // create edges between the new segments
        for (this, next) in result.iter().zip(result.iter().skip(1)) {
            self.create_edge(Edge(*this, *next));
        }

        // update paths and path occurrences
        let affected_paths: Vec<(_, _)> = self
            .get_node_unchecked(&handle.id())
            .occurrences
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();

        for (path_id, ix) in affected_paths.into_iter() {
            let step = path::StepIx::Step(ix);
            self.path_rewrite_segment(path_id, step, step, &result);
        }

        result
    }

    fn apply_orientation(&mut self, handle: Handle) -> Handle {
        if !handle.is_reverse() {
            return handle;
        }

        let node = self.get_node_mut(&handle.id()).unwrap();
        node.sequence = dna::rev_comp(node.sequence.as_slice()).into();

        let edges = {
            let node = self.get_node(&handle.id()).unwrap();
            node.left_edges
                .iter()
                .chain(node.right_edges.iter())
                .copied()
                .collect::<Vec<_>>()
        };

        for target in edges {
            let other = self.get_node_mut(&target.id()).unwrap();
            let backward_edges = if target.is_reverse() {
                other.right_edges.iter_mut()
            } else {
                other.left_edges.iter_mut()
            };

            for backward_handle in backward_edges {
                if backward_handle.id() == handle.id() {
                    *backward_handle = backward_handle.flip();
                    break;
                }
            }
        }

        let node = self.get_node_mut(&handle.id()).unwrap();
        std::mem::swap(&mut node.left_edges, &mut node.right_edges);

        let occurrences = &self.graph.get(&handle.id()).unwrap().occurrences;
        let paths = &mut self.paths;

        for (path_id, index) in occurrences.iter() {
            let path = paths.get_mut(&path_id).unwrap();
            let step = path.nodes.get_mut(*index).unwrap();
            *step = step.flip();
        }

        handle.flip()
    }
}

impl GraphPaths for HashGraph {
    type StepIx = path::StepIx;

    fn path_count(&self) -> usize {
        self.paths.len()
    }

    fn path_len(&self, id: PathId) -> Option<usize> {
        let path = self.paths.get(&id)?;
        Some(path.nodes.len())
    }

    fn path_circular(&self, id: PathId) -> Option<bool> {
        let path = self.paths.get(&id)?;
        Some(path.is_circular)
    }

    fn path_handle_at_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Handle> {
        let path = self.paths.get(&id)?;
        let handle = path.lookup_step_handle(&index)?;
        Some(handle)
    }

    fn path_first_step(&self, id: PathId) -> Option<Self::StepIx> {
        let _path = self.paths.get(&id)?;
        Some(path::StepIx::Front)
    }

    fn path_last_step(&self, id: PathId) -> Option<Self::StepIx> {
        let _path = self.paths.get(&id)?;
        Some(path::StepIx::End)
    }

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        match step {
            path::StepIx::Front => self.path_first_step(id),
            path::StepIx::End => self.path_last_step(id),
            path::StepIx::Step(ix) => {
                let len = self.path_len(id)?;
                if ix < len - 1 {
                    Some(path::StepIx::Step(ix + 1))
                } else {
                    self.path_last_step(id)
                }
            }
        }
    }

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        match step {
            path::StepIx::Front => Some(path::StepIx::Front),
            path::StepIx::End => {
                let len = self.path_len(id)?;
                Some(path::StepIx::Step(len - 1))
            }
            path::StepIx::Step(ix) => {
                if ix > 0 {
                    Some(path::StepIx::Step(ix - 1))
                } else {
                    Some(path::StepIx::Front)
                }
            }
        }
    }
}

impl<'a> GraphPathNames for &'a HashGraph {
    type PathName = std::iter::Copied<std::slice::Iter<'a, u8>>;

    fn get_path_id(self, name: &[u8]) -> Option<PathId> {
        self.path_id.get(name).copied()
    }

    fn get_path_name(self, id: PathId) -> Option<Self::PathName> {
        let path = self.paths.get(&id)?;
        Some(path.name.iter().copied())
    }
}

impl<'a> IntoPathIds for &'a HashGraph {
    type PathIds =
        std::iter::Copied<std::collections::hash_map::Keys<'a, PathId, Path>>;

    fn path_ids(self) -> Self::PathIds {
        self.paths.keys().copied()
    }
}

impl<'a> IntoNodeOccurrences for &'a HashGraph {
    type Occurrences = node::OccurIter<'a>;

    fn steps_on_handle(self, handle: Handle) -> Option<Self::Occurrences> {
        let node = self.get_node(&handle.id())?;
        let iter = node.occurrences.iter();
        Some(node::OccurIter { iter })
    }
}

impl<'a> GraphPathsRef for &'a HashGraph {
    type PathRef = &'a Path;

    fn get_path_ref(self, id: PathId) -> Option<&'a Path> {
        self.paths.get(&id)
    }
}

impl MutableGraphPaths for HashGraph {
    fn create_path(&mut self, name: &[u8], circular: bool) -> Option<PathId> {
        if self.path_id.contains_key(name) {
            return None;
        }

        let path_id = PathId(self.paths.len() as u64);
        let path = Path::new(name, path_id, circular);
        self.path_id.insert(name.into(), path_id);
        self.paths.insert(path_id, path);
        Some(path_id)
    }

    fn destroy_path(&mut self, id: PathId) -> bool {
        if let Some(path) = self.paths.get(&id) {
            for handle in path.nodes.iter() {
                let node: &mut Node = self.graph.get_mut(&handle.id()).unwrap();
                node.occurrences.remove(&id);
            }
            self.paths.remove(&id);
            true
        } else {
            false
        }
    }

    fn path_append_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx> {
        let path: &mut Path = self.paths.get_mut(&id)?;
        let node: &mut Node = self.graph.get_mut(&handle.id())?;

        path.nodes.push(handle);

        let step_offset = path.nodes.len() - 1;

        node.occurrences.insert(id, step_offset);

        Some(path::StepIx::Step(step_offset))
    }

    fn path_prepend_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx> {
        if !self.graph.contains_key(&handle.id()) {
            return None;
        }

        let path: &mut Path = self.paths.get_mut(&id)?;

        for h in path.nodes.iter() {
            let node: &mut Node = self.graph.get_mut(&h.id())?;
            let occurs = node.occurrences.get_mut(&id)?;
            *occurs += 1;
        }

        path.nodes.insert(0, handle);

        let step_offset = 0;
        let node: &mut Node = self.graph.get_mut(&handle.id())?;

        node.occurrences.insert(id, step_offset);

        Some(path::StepIx::Step(step_offset))
    }

    // TODO the offsets in here will probably need some fixing
    fn path_insert_step_after(
        &mut self,
        id: PathId,
        index: Self::StepIx,
        handle: Handle,
    ) -> Option<Self::StepIx> {
        if !self.graph.contains_key(&handle.id()) {
            return None;
        }

        let path: &mut Path = self.paths.get_mut(&id)?;
        let offset = match index {
            path::StepIx::Front => 0,
            path::StepIx::End => path.nodes.len() - 1,
            path::StepIx::Step(i) => (path.nodes.len() - 1).min(i + 1),
        };

        if offset < path.nodes.len() - 1 {
            for h in path.nodes[offset..].iter() {
                let node: &mut Node = self.graph.get_mut(&h.id())?;
                let occurs = node.occurrences.get_mut(&id)?;
                *occurs += 1;
            }
        }

        let inserted_offset = offset + 1;

        path.nodes.insert(inserted_offset, handle);

        let node: &mut Node = self.graph.get_mut(&handle.id())?;
        node.occurrences.insert(id, inserted_offset);

        Some(path::StepIx::Step(inserted_offset))
    }

    fn path_remove_step(
        &mut self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx> {
        let path: &mut Path = self.paths.get_mut(&id)?;

        // There's no step to remove at the indices before or after the path
        let to_remove = match index {
            path::StepIx::Front => None,
            path::StepIx::End => None,
            path::StepIx::Step(i) => Some(i),
        }?;

        if to_remove < path.nodes.len() - 1 {
            for h in path.nodes[(to_remove + 1)..].iter() {
                let node: &mut Node = self.graph.get_mut(&h.id())?;
                let occurs = node.occurrences.get_mut(&id)?;
                *occurs -= 1;
            }
        }

        let handle = path.nodes.remove(to_remove);

        let node: &mut Node = self.graph.get_mut(&handle.id())?;
        node.occurrences.remove(&id);

        Some(path::StepIx::Step(to_remove))
    }

    fn path_flip_step(
        &mut self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx> {
        let path: &mut Path = self.paths.get_mut(&id)?;

        // There's no step to alter at the indices before or after the path
        let to_flip = match index {
            path::StepIx::Front => None,
            path::StepIx::End => None,
            path::StepIx::Step(i) => Some(i),
        }?;

        let handle = path.nodes.get_mut(to_flip)?;
        *handle = handle.flip();

        Some(index)
    }

    fn path_rewrite_segment(
        &mut self,
        id: PathId,
        from: Self::StepIx,
        to: Self::StepIx,
        new_segment: &[Handle],
    ) -> Option<(Self::StepIx, Self::StepIx)> {
        let path: &mut Path = self.paths.get_mut(&id)?;

        let start = path.step_index_offset(from);
        let end = path.step_index_offset(to);

        if end < start {
            return None;
        }

        {
            let graph = &mut self.graph;

            path.nodes
                .iter()
                .skip(start)
                .take(end - start + 1)
                .for_each(|handle| {
                    let node = graph.get_mut(&handle.id()).unwrap();
                    node.occurrences.remove(&id);
                });
        }

        path.nodes.splice(start..=end, new_segment.iter().copied());

        {
            let graph = &mut self.graph;

            path.nodes.iter().enumerate().for_each(|(ix, handle)| {
                let node = graph.get_mut(&handle.id()).unwrap();
                node.occurrences.insert(id, ix);
            });
        }

        let start_step = path::StepIx::Step(start);
        let end_step = path::StepIx::Step(start + new_segment.len() - 1);

        Some((start_step, end_step))
    }

    fn path_set_circularity(
        &mut self,
        id: PathId,
        circular: bool,
    ) -> Option<()> {
        let path: &mut Path = self.paths.get_mut(&id)?;
        path.is_circular = circular;
        Some(())
    }
}

impl PathSequences for HashGraph {
    fn path_bases_len(&self, id: PathId) -> Option<usize> {
        let path = self.paths.get(&id)?;
        let len = path
            .nodes
            .iter()
            .filter_map(|h| self.graph.get(&h.id()).map(|n| n.sequence.len()))
            .sum();
        Some(len)
    }

    fn path_step_at_base(
        &self,
        id: PathId,
        pos: usize,
    ) -> Option<Self::StepIx> {
        let path = self.paths.get(&id)?;
        Some(path.step_at_position(&self.graph, pos))
    }

    fn path_step_base_offset(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<usize> {
        let path = self.paths.get(&id)?;
        path.position_of_step(&self.graph, index)
    }
}
