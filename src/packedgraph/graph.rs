use fnv::{FnvHashMap, FnvHashSet};

use rayon::prelude::*;

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::IntoNeighbors,
    mutablehandlegraph::*,
    packed::traits::*,
    pathhandlegraph::PathId,
};

pub(super) use super::{
    edges::EdgeLists,
    index::{NodeRecordId, OneBasedIndex, RecordIndex},
    nodes::NodeRecords,
    occurrences::NodeOccurrences,
    paths::PackedGraphPaths,
    sequence::SeqRecordIx,
};

use super::edges::EdgeListIx;
use super::occurrences::OccurListIx;

use super::{defragment::Defragment, paths};

#[allow(unused_imports)]
use log::{debug, error, info, trace};

pub(crate) static NARROW_PAGE_WIDTH: usize = 256;
pub(crate) static WIDE_PAGE_WIDTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub nodes: NodeRecords,
    pub edges: EdgeLists,
    pub occurrences: NodeOccurrences,
    pub paths: PackedGraphPaths,
}

crate::impl_space_usage!(PackedGraph, [nodes, edges, occurrences, paths]);

impl Default for PackedGraph {
    fn default() -> Self {
        let nodes = Default::default();
        let edges = Default::default();
        let occurrences = Default::default();
        let paths = Default::default();
        PackedGraph {
            nodes,
            edges,
            occurrences,
            paths,
        }
    }
}

impl Defragment for PackedGraph {
    type Updates = ();

    fn defragment(&mut self) -> Option<()> {
        // Defragment the paths
        let paths_update = self.paths.defragment();

        // Defragment the occurrences
        let occurs_update = self.occurrences.defragment();
        // Update the new occurrences using the paths update
        if let Some(paths_update) = paths_update {
            self.occurrences.apply_path_updates(&paths_update);
        }

        // Defragment the edges
        let edges_update = self.edges.defragment();
        // Defragment the nodes
        let _nodes_update = self.nodes.defragment();

        // Update the nodes using the occurrence and edge updates
        match (occurs_update, edges_update) {
            (None, None) => (),
            (Some(occurs_update), None) => {
                self.nodes.apply_node_occur_ix_updates(&occurs_update);
            }
            (None, Some(edges_update)) => {
                self.nodes.apply_edge_lists_ix_updates(&edges_update);
            }
            (Some(occurs_update), Some(edges_update)) => {
                self.nodes.apply_edge_and_occur_updates(
                    &edges_update,
                    &occurs_update,
                );
            }
        }

        Some(())
    }
}

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_expected_node_count(nodes: usize) -> Self {
        let nodes = NodeRecords::with_expected_node_count(nodes);
        Self {
            nodes,
            ..Default::default()
        }
    }

    pub fn create_edges_iter<I>(&mut self, mut iter: I)
    where
        I: Iterator<Item = Edge>,
    {
        let edge_page_size = self.edges.record_vec.page_size();

        let mut page_buf: Vec<u64> = Vec::with_capacity(edge_page_size);
        let mut data_buf: Vec<u64> = Vec::with_capacity(edge_page_size);
        let mut edge_vec_ix = 1 + (self.edges.record_vec.len() / 2);

        while let Some(Edge(left, right)) = iter.next() {
            let left_gix =
                self.nodes.handle_record(left).unwrap_or_else(|| {
                    panic!("handle {} does not have a record", left.0)
                });
            let right_gix =
                self.nodes.handle_record(right).unwrap_or_else(|| {
                    panic!("handle {} does not have a record", right.0)
                });

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

            let left_edge_list =
                self.nodes.get_edge_list(left_gix, left_edge_dir);
            let right_edge_list =
                self.nodes.get_edge_list(right_gix, right_edge_dir);

            data_buf.push(right.pack());
            data_buf.push(left_edge_list.pack());

            self.nodes.set_edge_list(
                left_gix,
                left_edge_dir,
                EdgeListIx::from_one_based(edge_vec_ix),
            );
            edge_vec_ix += 1;

            data_buf.push(left.flip().pack());
            data_buf.push(right_edge_list.pack());
            self.nodes.set_edge_list(
                right_gix,
                right_edge_dir,
                EdgeListIx::from_one_based(edge_vec_ix),
            );
            edge_vec_ix += 1;

            if data_buf.len() >= edge_page_size {
                self.edges.record_vec.append_pages(&mut page_buf, &data_buf);
                data_buf.clear();
            }
        }

        if !data_buf.is_empty() {
            self.edges.record_vec.append_pages(&mut page_buf, &data_buf);
            data_buf.clear();
        }
    }

    pub(crate) fn remove_edge_from(
        &mut self,
        on: Handle,
        to: Handle,
    ) -> Option<()> {
        let edge_dir = if on.is_reverse() {
            Direction::Left
        } else {
            Direction::Right
        };

        let gix = self.nodes.handle_record(on).unwrap();

        let edge_list = self.nodes.get_edge_list(gix, edge_dir);

        let new_head = self
            .edges
            .iter_mut(edge_list)
            .remove_record_with(|_, (h, _)| h == to)?;

        if new_head != edge_list {
            trace!(
                "updating edge list head for {}, from {} to {}",
                on.0,
                edge_list.pack(),
                new_head.pack()
            );
            self.nodes.set_edge_list(gix, edge_dir, new_head);
        }

        if on == to.flip() {
            self.edges.removed_reversing_self_edge_records += 1;
        }

        Some(())
    }

    pub(super) fn remove_path_impl(&mut self, id: PathId) -> Option<()> {
        let step_updates = self.paths.remove_path(id)?;

        self.apply_node_occurrences_iter(id, step_updates);

        Some(())
    }

    pub(super) fn remove_handle_impl(&mut self, handle: Handle) -> Option<()> {
        let rec_ix = self.nodes.handle_record(handle)?;

        // Collect all the path IDs that this node is on, and all the
        // occurrence list indices
        let occur_head = self.nodes.node_record_occur(rec_ix)?;

        let path_ids: FnvHashSet<_> = self
            .occurrences
            .iter(occur_head)
            .map(|(_occ_ix, record)| record.path_id)
            .collect();

        // Remove those paths
        for path in path_ids {
            self.remove_path_impl(path);
        }

        // Remove the occurrences
        let occurs: Vec<_> = self
            .occurrences
            .iter(occur_head)
            .map(|(ptr, _)| ptr)
            .collect::<Vec<_>>();
        for ptr in occurs {
            if let Some(ix) = ptr.to_zero_based() {
                self.occurrences.path_ids.set(ix, 0);
                self.occurrences.node_occur_offsets.set(ix, 0);
                self.occurrences.node_occur_next.set(ix, 0);
                self.occurrences.removed_records += 1;
            }
        }

        // Remove the left and right edges of the node
        let lefts = self.neighbors(handle, Direction::Left).collect::<Vec<_>>();

        let rights =
            self.neighbors(handle, Direction::Right).collect::<Vec<_>>();

        trace!(
            "removing handle {} with {} left and {} right neighbors",
            handle.0,
            lefts.len(),
            rights.len()
        );

        for prev in lefts {
            trace!("remove_edge_from({}, {})", prev.0, handle.0);
            self.remove_edge_from(prev, handle);

            if prev != handle.flip() {
                self.remove_edge_from(handle.forward().flip(), prev.flip());
            }
        }

        for next in rights {
            trace!("remove_edge_from({}, {})", next.flip().0, handle.flip().0);
            self.remove_edge_from(next.flip(), handle.flip());

            if next != handle.flip() {
                self.remove_edge_from(handle.forward(), next);
            }
        }

        self.nodes.clear_node_record(handle.id())?;

        Some(())
    }

    pub(super) fn apply_step_updates_worker<'a>(
        receiver: crossbeam_channel::Receiver<(PathId, paths::StepUpdate)>,
        nodes: &'a mut NodeRecords,
        occurrences: &'a mut NodeOccurrences,
    ) {
        use paths::{StepPtr, StepUpdate};

        let path_id_page_size = occurrences.path_ids.page_size();
        let offsets_page_size = occurrences.node_occur_offsets.page_size();
        let next_ptr_page_size = occurrences.node_occur_next.page_size();

        let mut buf: Vec<u64> = Vec::with_capacity(1024);

        let mut path_id_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut offset_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut next_ptr_buf: Vec<u64> = Vec::with_capacity(1024);

        let mut to_remove: Vec<(PathId, usize, OccurListIx, StepPtr)> =
            Vec::with_capacity(128);

        let mut new_occur_ix = occurrences.path_ids.len() + 1;

        while let Ok((path_id, step_update)) = receiver.recv() {
            match step_update {
                StepUpdate::Insert { handle, step } => {
                    let rec_id = nodes.handle_record(handle).unwrap();
                    let vec_ix = rec_id.to_zero_based().unwrap();

                    let occur_ix: OccurListIx =
                        nodes.node_occurrence_map.get_unpack(vec_ix);

                    path_id_buf.push(path_id.pack());
                    offset_buf.push(step.pack());
                    next_ptr_buf.push(occur_ix.pack());

                    nodes.node_occurrence_map.set_pack(vec_ix, new_occur_ix);
                    new_occur_ix += 1;
                }
                StepUpdate::Remove { handle, step } => {
                    let rec_id = nodes.handle_record(handle).unwrap();
                    let vec_ix = rec_id.to_zero_based().unwrap();

                    let occur_head =
                        nodes.node_occurrence_map.get_unpack(vec_ix);

                    to_remove.push((path_id, vec_ix, occur_head, step));
                }
            }

            if path_id_buf.len() >= path_id_page_size {
                occurrences.path_ids.append_pages(&mut buf, &path_id_buf);

                path_id_buf.clear();
            }

            if offset_buf.len() >= offsets_page_size {
                occurrences
                    .node_occur_offsets
                    .append_pages(&mut buf, &offset_buf);

                offset_buf.clear();
            }

            if next_ptr_buf.len() >= next_ptr_page_size {
                occurrences
                    .node_occur_next
                    .append_pages(&mut buf, &next_ptr_buf);

                next_ptr_buf.clear();
            }

            if to_remove.len() >= 124 {
                // TODO have to handle causality within each path here
                for &(path_id, vec_ix, occur_head, step) in to_remove.iter() {
                    let new_occur_ix = occurrences
                        .iter_mut(occur_head)
                        .remove_record_with(|_, record| {
                            record.path_id == path_id && record.offset == step
                        });

                    if let Some(new_head) = new_occur_ix {
                        if new_head != occur_head {
                            nodes
                                .node_occurrence_map
                                .set_pack(vec_ix, new_head);
                        }
                    }
                }

                to_remove.clear();
            }
        }

        occurrences.path_ids.append_pages(&mut buf, &path_id_buf);

        occurrences
            .node_occur_offsets
            .append_pages(&mut buf, &offset_buf);

        occurrences
            .node_occur_next
            .append_pages(&mut buf, &next_ptr_buf);

        // TODO have to handle causality within each path here
        for &(path_id, vec_ix, occur_head, step) in to_remove.iter() {
            let new_occur_ix = occurrences
                .iter_mut(occur_head)
                .remove_record_with(|_, record| {
                    record.path_id == path_id && record.offset == step
                });

            if let Some(new_head) = new_occur_ix {
                if new_head != occur_head {
                    nodes.node_occurrence_map.set_pack(vec_ix, new_head);
                }
            }
        }
    }

    pub(super) fn apply_node_occurrence_consumer<'a>(
        receiver: crossbeam_channel::Receiver<(PathId, Vec<paths::StepUpdate>)>,
        nodes: &'a mut NodeRecords,
        occurrences: &'a mut NodeOccurrences,
    ) {
        use paths::{StepPtr, StepUpdate};

        let mut buf: Vec<u64> = Vec::with_capacity(1024);

        let mut path_id_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut offset_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut next_ptr_buf: Vec<u64> = Vec::with_capacity(1024);

        let mut to_remove: Vec<(usize, OccurListIx, StepPtr)> =
            Vec::with_capacity(124);

        while let Ok((path_id, updates)) = receiver.recv() {
            path_id_buf.clear();
            offset_buf.clear();
            next_ptr_buf.clear();
            to_remove.clear();

            let mut new_occur_ix = occurrences.path_ids.len() + 1;

            for step_update in updates {
                match step_update {
                    StepUpdate::Insert { handle, step } => {
                        let rec_id = nodes.handle_record(handle).unwrap();
                        let vec_ix = rec_id.to_zero_based().unwrap();

                        let occur_ix: OccurListIx =
                            nodes.node_occurrence_map.get_unpack(vec_ix);

                        path_id_buf.push(path_id.pack());
                        offset_buf.push(step.pack());
                        next_ptr_buf.push(occur_ix.pack());

                        nodes
                            .node_occurrence_map
                            .set_pack(vec_ix, new_occur_ix);
                        new_occur_ix += 1;
                    }
                    StepUpdate::Remove { handle, step } => {
                        let rec_id = nodes.handle_record(handle).unwrap();
                        let vec_ix = rec_id.to_zero_based().unwrap();

                        let occur_head =
                            nodes.node_occurrence_map.get_unpack(vec_ix);

                        to_remove.push((vec_ix, occur_head, step));
                    }
                }
            }
            occurrences.path_ids.append_pages(&mut buf, &path_id_buf);

            occurrences
                .node_occur_offsets
                .append_pages(&mut buf, &offset_buf);

            occurrences
                .node_occur_next
                .append_pages(&mut buf, &next_ptr_buf);

            for &(vec_ix, occur_head, step) in to_remove.iter() {
                let new_occur_ix = occurrences
                    .iter_mut(occur_head)
                    .remove_record_with(|_, record| {
                        record.path_id == path_id && record.offset == step
                    });

                if let Some(new_head) = new_occur_ix {
                    if new_head != occur_head {
                        nodes.node_occurrence_map.set_pack(vec_ix, new_head);
                    }
                }
            }
        }
    }

    pub(super) fn apply_node_occurrence(
        &mut self,
        path_id: PathId,
        step_update: paths::StepUpdate,
    ) {
        use paths::StepUpdate;
        match step_update {
            StepUpdate::Insert { handle, step } => {
                let rec_id = self.nodes.handle_record(handle).unwrap();
                let vec_ix = rec_id.to_zero_based().unwrap();

                let occur_ix =
                    self.nodes.node_occurrence_map.get_unpack(vec_ix);

                let new_occur_ix =
                    self.occurrences.append_entry(path_id, step, occur_ix);

                self.nodes
                    .node_occurrence_map
                    .set_pack(vec_ix, new_occur_ix);
            }
            StepUpdate::Remove { handle, step } => {
                let rec_id = self.nodes.handle_record(handle).unwrap();
                let vec_ix = rec_id.to_zero_based().unwrap();

                let occur_head =
                    self.nodes.node_occurrence_map.get_unpack(vec_ix);

                let new_occur_ix = self
                    .occurrences
                    .iter_mut(occur_head)
                    .remove_record_with(|_, record| {
                        record.path_id == path_id && record.offset == step
                    });

                if let Some(new_head) = new_occur_ix {
                    if new_head != occur_head {
                        self.nodes
                            .node_occurrence_map
                            .set_pack(vec_ix, new_head);
                    }
                }
            }
        }
    }

    pub(super) fn apply_node_occurrences_iter<I>(
        &mut self,
        path_id: PathId,
        iter: I,
    ) where
        I: IntoIterator<Item = paths::StepUpdate>,
    {
        iter.into_iter()
            .for_each(|s| self.apply_node_occurrence(path_id, s))
    }

    pub fn with_path_mut_ctx<F>(&mut self, path_id: PathId, f: F)
    where
        for<'b> F: Fn(&mut paths::PackedPathMut<'b>) -> Vec<paths::StepUpdate>,
    {
        let steps = self.paths.with_path_mut_ctx(path_id, f);
        if let Some(steps) = steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub fn zip_all_paths_mut_ctx<T, I, F>(&mut self, iter: I, f: F)
    where
        I: IndexedParallelIterator<Item = T>,
        T: Send + Sync,
        for<'b> F: Fn(
                T,
                PathId,
                &mut paths::PackedPathMut<'b>,
            ) -> Vec<paths::StepUpdate>
            + Send
            + Sync,
    {
        let all_steps = self.paths.zip_with_paths_mut_ctx(iter, f);
        for (path_id, steps) in all_steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub fn with_all_paths_mut_ctx<F>(&mut self, f: F)
    where
        for<'b> F: Fn(PathId, &mut paths::PackedPathMut<'b>) -> Vec<paths::StepUpdate>
            + Sync
            + Send,
    {
        let all_steps = self.paths.with_all_paths_mut_ctx_par(f);
        for (path_id, steps) in all_steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub fn with_all_paths_mut_ctx_chn_new<F>(&mut self, f: F)
    where
        for<'b> F: Fn(
                PathId,
                &mut crossbeam_channel::Sender<(PathId, paths::StepUpdate)>,
                &mut paths::PackedPathMut<'b>,
            ) + Sync,
    {
        use crossbeam_channel::unbounded;

        let (sender, receiver) = unbounded::<(PathId, paths::StepUpdate)>();
        // unbounded::<(PathId, Vec<paths::StepUpdate>)>();

        let paths = &mut self.paths;
        let nodes = &mut self.nodes;
        let occurrences = &mut self.occurrences;

        rayon::join(
            || {
                Self::apply_step_updates_worker(receiver, nodes, occurrences);
            },
            || {
                let mut mut_ctx = paths.get_all_paths_mut_ctx();
                let refs_mut = mut_ctx.par_iter_mut();

                refs_mut.for_each_with(sender, |s, path| {
                    let path_id = path.path_id;
                    f(path_id, s, path);
                });
            },
        );
    }

    pub fn with_all_paths_mut_ctx_chn<F>(&mut self, f: F)
    where
        for<'b> F: Fn(PathId, &mut paths::PackedPathMut<'b>) -> Vec<paths::StepUpdate>
            + Sync,
    {
        use crossbeam_channel::unbounded;

        let (sender, receiver) =
            unbounded::<(PathId, Vec<paths::StepUpdate>)>();

        let paths = &mut self.paths;
        let nodes = &mut self.nodes;
        let occurrences = &mut self.occurrences;

        rayon::join(
            || {
                Self::apply_node_occurrence_consumer(
                    receiver,
                    nodes,
                    occurrences,
                );
            },
            || {
                let mut mut_ctx = paths.get_all_paths_mut_ctx();
                let refs_mut = mut_ctx.par_iter_mut();

                refs_mut.for_each_with(sender, |s, path| {
                    let path_id = path.path_id;
                    let updates = f(path_id, path);
                    s.send((path_id, updates)).unwrap();
                });
            },
        );
    }

    pub fn compact_ids(&mut self) {
        let mut seen_nodes: FnvHashMap<NodeId, NodeId> = FnvHashMap::default();
        let mut next_id = 1u64;

        let id_fun = move |node_in: NodeId| -> NodeId {
            if let Some(new_id) = seen_nodes.get(&node_in) {
                *new_id
            } else {
                let new_id = NodeId::from(next_id);
                next_id += 1;
                seen_nodes.insert(node_in, new_id);
                new_id
            }
        };

        self.transform_node_ids_mut(id_fun);
    }

    pub fn trace_edge_status(&self) {
        use crate::handlegraph::IntoEdges;
        use crate::handlegraph::IntoHandles;

        let length = self.edges.record_count();
        let edge_count = self.edges.len();
        let edge_iter_count = self.edges().count();

        let mut missing = 0;
        let mut missing_and_zero = 0;
        let mut good = 0;

        for ix in 0..length {
            let tgt_ix = 2 * ix;
            let handle: Handle = self.edges.record_vec.get_unpack(tgt_ix);
            let n_id = handle.id();

            if n_id.is_zero() && !self.has_node(n_id) {
                missing_and_zero += 1;
            }

            if !self.has_node(n_id) && !n_id.is_zero() {
                missing += 1;
            } else if !n_id.is_zero() && self.has_node(n_id) {
                good += 1;
            }
        }

        info!("edge count:           {}", edge_count);
        info!("edge iter count:      {}", edge_iter_count);
        info!("edge count * 2:       {}", edge_count * 2);
        info!("record count:         {}", length);
        info!("deleted records:      {}", self.edges.removed_count);
        info!("");
        info!("missing & zero edges: {}", missing_and_zero);
        info!("missing edge targets: {}", missing);
        info!("good edges:           {}", good);
        info!("------------------------------------");
    }

    // pub fn neighbors_trace<'a>(&'a self, handle: Handle, dir: Direction) -> super::iter::EdgeListHandleIterTrace<'a> {
    pub fn neighbors_trace<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
    ) -> super::iter::EdgeListHandleIterTrace<'a> {
        use crate::handlegraph::IntoHandles;
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

        super::iter::EdgeListHandleIterTrace::new(iter, dir == Dir::Left)
    }

    pub fn neighbors_trace_continue<'a>(
        &'a self,
        handle: Handle,
        dir: Direction,
        visited: FnvHashSet<EdgeListIx>,
    ) -> super::iter::EdgeListHandleIterTrace<'a> {
        use crate::handlegraph::IntoHandles;
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

        super::iter::EdgeListHandleIterTrace::new_continue(
            iter,
            dir == Dir::Left,
            visited,
        )
    }

    pub fn visitable_edges(&self) -> FnvHashSet<EdgeListIx> {
        use crate::handlegraph::IntoHandles;
        let mut visited: FnvHashSet<EdgeListIx> = FnvHashSet::default();

        for handle in self.handles() {
            self.neighbors_edge_ixs(handle, &mut visited);
        }

        visited
    }

    pub fn zero_edges_partition(
        &self,
    ) -> (FnvHashSet<EdgeListIx>, FnvHashSet<EdgeListIx>) {
        let mut zero: FnvHashSet<EdgeListIx> = FnvHashSet::default();
        let mut non_zero: FnvHashSet<EdgeListIx> = FnvHashSet::default();

        let total_records = self.edges.record_vec.len() / 2;

        for ix in 0..total_records {
            let edge_ix = EdgeListIx::from_zero_based(ix);
            let edge_vec_ix = edge_ix.to_record_ix(2, 0).unwrap();
            let handle = self.edges.record_vec.get(edge_vec_ix);

            if handle == 0 {
                zero.insert(edge_ix);
            } else {
                non_zero.insert(edge_ix);
            }
        }

        (non_zero, zero)
    }

    pub fn neighbors_edge_ixs(
        &self,
        handle: Handle,
        visited: &mut FnvHashSet<EdgeListIx>,
    ) {
        use crate::handlegraph::IntoHandles;

        if !self.has_node(handle.id()) {
            panic!(
                "tried to get neighbors of node {} which doesn't exist",
                handle.id().0
            );
        }
        let g_ix = self.nodes.handle_record(handle).unwrap();

        let edge_list_ix = if handle.is_reverse() {
            self.nodes.get_edge_list(g_ix, Direction::Right)
        } else {
            self.nodes.get_edge_list(g_ix, Direction::Left)
        };

        let iter = self.edges.iter(edge_list_ix);

        super::iter::EdgeListHandleIterTrace::visit_now(iter, true, visited);

        let edge_list_ix = if handle.is_reverse() {
            self.nodes.get_edge_list(g_ix, Direction::Left)
        } else {
            self.nodes.get_edge_list(g_ix, Direction::Right)
        };

        let iter = self.edges.iter(edge_list_ix);

        super::iter::EdgeListHandleIterTrace::visit_now(iter, false, visited);
    }
}
