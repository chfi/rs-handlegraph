use fnv::FnvHashSet;

use rayon::prelude::*;

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::IntoNeighbors,
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

use super::occurrences::OccurListIx;

use super::{defragment::Defragment, paths};

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

    pub(super) fn transform_node_ids<F>(&mut self, transform: F)
    where
        F: Fn(NodeId) -> NodeId + Copy + Send + Sync,
    {
        // Create a new NodeIdIndexMap
        self.nodes.transform_node_ids(transform);

        // Update the targets of all edges
        self.edges.transform_targets(transform);

        // Update the steps of all paths
        self.with_all_paths_mut_ctx(|_, path_ref| {
            path_ref.path.transform_steps(transform);
            Vec::new()
        });
    }

    pub(super) fn remove_edge_impl(&mut self, edge: Edge) -> Option<()> {
        let Edge(left, right) = edge;

        let left_gix = self.nodes.handle_record(left)?;

        let right_gix = self.nodes.handle_record(right)?;

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

        // remove the edge from `left`'s edge list
        let new_left_head = self
            .edges
            .iter_mut(left_edge_list)
            .remove_record_with(|_, (handle, _)| handle == right)?;

        // remove the edge from `right`'s edge list
        let new_right_head = self
            .edges
            .iter_mut(right_edge_list)
            .remove_record_with(|_, (handle, _)| handle == left)?;

        // update `left`'s edge list header
        self.nodes
            .set_edge_list(left_gix, left_edge_dir, new_left_head);

        // update `right`'s edge list header
        self.nodes
            .set_edge_list(right_gix, right_edge_dir, new_right_head);

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
        self.occurrences
            .iter_mut(occur_head)
            .remove_all_records_with(|_, _| true);

        // Remove the left and right edges of the node
        let lefts = self.neighbors(handle, Direction::Left).collect::<Vec<_>>();

        let rights =
            self.neighbors(handle, Direction::Right).collect::<Vec<_>>();

        for other in lefts {
            let edge = Edge(other, handle);
            self.remove_edge_impl(edge);
        }

        for other in rights {
            let edge = Edge(handle, other);
            self.remove_edge_impl(edge);
        }

        self.nodes.clear_node_record(handle.id())?;

        Some(())
    }

    pub(super) fn apply_node_occurrence_consumer_new<'a>(
        receiver: crossbeam_channel::Receiver<(PathId, Vec<paths::StepUpdate>)>,
        nodes: &'a mut NodeRecords,
        occurrences: &'a mut NodeOccurrences,
    ) {
        use paths::{StepPtr, StepUpdate};

        let mut buf: Vec<u64> = Vec::with_capacity(1024);

        let mut path_id_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut offset_buf: Vec<u64> = Vec::with_capacity(1024);
        let mut next_ptr_buf: Vec<u64> = Vec::with_capacity(1024);

        let mut to_remove: Vec<(OccurListIx, StepPtr)> =
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

                        to_remove.push((occur_head, step));
                    }
                }
            }

            let path_ids_rest = if !occurrences.path_ids.pages_full() {
                occurrences
                    .path_ids
                    .fill_last_page(&mut buf, &path_id_buf)
                    .unwrap()
            } else {
                path_id_buf.as_slice()
            };

            let mut rest = path_ids_rest;
            if !path_ids_rest.is_empty() {
                loop {
                    match occurrences.path_ids.append_page(rest) {
                        None => break,
                        Some(new_rest) => {
                            // if new_rest.is_empty() || new {
                            if new_rest.len() < 1024 {
                                break;
                            }
                            rest = new_rest;
                        }
                    }
                }
            }

            occurrences.path_ids.fill_last_page(&mut buf, rest);

            let offsets_rest = if !occurrences.node_occur_offsets.pages_full() {
                occurrences
                    .node_occur_offsets
                    .fill_last_page(&mut buf, &offset_buf)
                    .unwrap()
            } else {
                offset_buf.as_slice()
            };

            let mut rest = offsets_rest;
            if !offsets_rest.is_empty() {
                loop {
                    match occurrences.node_occur_offsets.append_page(rest) {
                        None => break,
                        Some(new_rest) => {
                            if new_rest.len() < 256 {
                                // if new_rest.is_empty() {
                                break;
                            }
                            rest = new_rest;
                        }
                    }
                }
            }

            occurrences
                .node_occur_offsets
                .fill_last_page(&mut buf, rest);

            let next_ptr_rest = if !occurrences.node_occur_next.pages_full() {
                occurrences
                    .node_occur_next
                    .fill_last_page(&mut buf, &next_ptr_buf)
                    .unwrap()
            } else {
                next_ptr_buf.as_slice()
            };

            let mut rest = next_ptr_rest;
            if !next_ptr_rest.is_empty() {
                loop {
                    match occurrences.node_occur_next.append_page(rest) {
                        None => break,
                        Some(new_rest) => {
                            // if new_rest.is_empty() {
                            if new_rest.len() < 256 {
                                break;
                            }
                            rest = new_rest;
                        }
                    }
                }
            }

            occurrences.node_occur_next.fill_last_page(&mut buf, rest);
        }
    }

    pub(super) fn apply_node_occurrence_consumer<'a>(
        receiver: crossbeam_channel::Receiver<(PathId, Vec<paths::StepUpdate>)>,
        nodes: &'a mut NodeRecords,
        occurrences: &'a mut NodeOccurrences,
    ) {
        use paths::StepUpdate;
        while let Ok((path_id, updates)) = receiver.recv() {
            for step_update in updates {
                match step_update {
                    StepUpdate::Insert { handle, step } => {
                        let rec_id = nodes.handle_record(handle).unwrap();
                        let vec_ix = rec_id.to_zero_based().unwrap();

                        let occur_ix =
                            nodes.node_occurrence_map.get_unpack(vec_ix);

                        let new_occur_ix =
                            occurrences.append_entry(path_id, step, occur_ix);

                        nodes
                            .node_occurrence_map
                            .set_pack(vec_ix, new_occur_ix);
                    }
                    StepUpdate::Remove { handle, step } => {
                        let rec_id = nodes.handle_record(handle).unwrap();
                        let vec_ix = rec_id.to_zero_based().unwrap();

                        let occur_head =
                            nodes.node_occurrence_map.get_unpack(vec_ix);

                        let new_occur_ix = occurrences
                            .iter_mut(occur_head)
                            .remove_record_with(|_, record| {
                                record.path_id == path_id
                                    && record.offset == step
                            });

                        if let Some(new_head) = new_occur_ix {
                            if new_head != occur_head {
                                nodes
                                    .node_occurrence_map
                                    .set_pack(vec_ix, new_head);
                            }
                        }
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
                Self::apply_node_occurrence_consumer_new(
                    receiver,
                    nodes,
                    occurrences,
                );
                // Self::apply_node_occurrence_consumer(
                //     receiver,
                //     nodes,
                //     occurrences,
                // );
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
}
