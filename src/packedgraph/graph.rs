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

    /*
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
    */

    pub fn create_edges_iter<I>(&mut self, mut iter: I)
    where
        I: Iterator<Item = Edge>,
    {
        let edge_page_size = self.edges.record_vec.page_size();

        let mut page_buf: Vec<u64> = Vec::with_capacity(edge_page_size);
        let mut data_buf: Vec<u64> = Vec::with_capacity(edge_page_size);
        let mut edge_vec_ix = 1 + (self.edges.record_vec.len() / 2);

        while let Some(Edge(left, right)) = iter.next() {
            // let left_gix = self.nodes.handle_record(left).unwrap();
            // let right_gix = self.nodes.handle_record(right).unwrap();
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

        let gix = self.nodes.handle_record(on)?;

        let edge_list = self.nodes.get_edge_list(gix, edge_dir);
        let new_head = self
            .edges
            .iter_mut(edge_list)
            .remove_record_with(|_, (h, _)| h == to)?;

        if new_head != edge_list {
            self.nodes.set_edge_list(gix, edge_dir, new_head);
        }

        Some(())
    }

    pub(super) fn remove_edge_impl(&mut self, edge: Edge) -> Option<()> {
        unimplemented!();
        /*
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
            .remove_record_with(|_, (handle, _)| handle.id() == right.id())?;

        // remove the edge from `right`'s edge list
        let new_right_head = self
            .edges
            .iter_mut(right_edge_list)
            .remove_record_with(|_, (handle, _)| handle.id() == left.id())?;

        // update `left`'s edge list header
        self.nodes
            .set_edge_list(left_gix, left_edge_dir, new_left_head);

        // update `right`'s edge list header
        self.nodes
            .set_edge_list(right_gix, right_edge_dir, new_right_head);

        Some(())
            */
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

        // self.occurrences
        //     .iter_mut(occur_head)
        //     .remove_all_records_with(|_, _| true);

        // Remove the left and right edges of the node
        let lefts = self.neighbors(handle, Direction::Left).collect::<Vec<_>>();

        let rights =
            self.neighbors(handle, Direction::Right).collect::<Vec<_>>();

        info!(
            "removing handle {} with {} left and {} right neighbors",
            handle.0,
            lefts.len(),
            rights.len()
        );

        for prev in lefts {
            info!("remove_edge_from({}, {})", prev.0, handle.0);
            self.remove_edge_from(prev, handle);
        }

        for next in rights {
            info!("remove_edge_from({}, {})", next.flip().0, handle.flip().0);
            self.remove_edge_from(next.flip(), handle.flip());
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
}
