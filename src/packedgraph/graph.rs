#[allow(unused_imports)]
pub(super) use super::{
    edges::{EdgeListIx, EdgeLists, EdgeVecIx},
    index::{NodeRecordId, OneBasedIndex, RecordIndex},
    nodes::{GraphVecIx, NodeIdIndexMap, NodeRecords},
    occurrences::{NodeOccurrences, OccurListIx, OccurRecord},
    paths::{PackedGraphPaths, PathStepIx},
    sequence::{PackedSeqIter, SeqRecordIx, Sequences},
};

use crate::handle::{Handle, NodeId};

use crate::pathhandlegraph::PathId;

use crate::packed::traits::*;

use super::list;
use super::list::{PackedList, PackedListMut};

use super::paths;

pub(crate) static NARROW_PAGE_WIDTH: usize = 256;
pub(crate) static WIDE_PAGE_WIDTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub(super) nodes: NodeRecords,
    pub(super) edges: EdgeLists,
    pub(super) occurrences: NodeOccurrences,
    pub(super) paths: PackedGraphPaths,
}

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

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub(super) fn apply_node_occurrence_consumer<'a>(
        receiver: std::sync::mpsc::Receiver<(PathId, Vec<paths::StepUpdate>)>,
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

    pub(super) fn with_path_mut_ctx<'a, F>(&'a mut self, path_id: PathId, f: F)
    where
        for<'b> F:
            Fn(&mut paths::PackedPathRefMut<'b>) -> Vec<paths::StepUpdate>,
    {
        let steps = self.paths.with_path_mut_ctx(path_id, f);
        if let Some(steps) = steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub(super) fn zip_all_paths_mut_ctx<'a, T, I, F>(
        &'a mut self,
        iter: I,
        f: F,
    ) where
        I: Iterator<Item = T>,
        for<'b> F: Fn(
            T,
            PathId,
            &mut paths::PackedPathRefMut<'b>,
        ) -> Vec<paths::StepUpdate>,
    {
        let all_steps = self.paths.zip_with_paths_mut_ctx(iter, f);
        for (path_id, steps) in all_steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub(super) fn with_all_paths_mut_ctx<'a, F>(&'a mut self, f: F)
    where
        for<'b> F: Fn(
                PathId,
                &mut paths::PackedPathRefMut<'b>,
            ) -> Vec<paths::StepUpdate>
            + Sync,
    {
        let all_steps = self.paths.with_multipath_mut_ctx_par(f);
        for (path_id, steps) in all_steps {
            self.apply_node_occurrences_iter(path_id, steps);
        }
    }

    pub(super) fn with_all_paths_mut_ctx_<F>(&mut self, f: F)
    where
        for<'b> F: Fn(
                PathId,
                &mut paths::PackedPathRefMut<'b>,
            ) -> Vec<paths::StepUpdate>
            + Sync,
    {
        use rayon::prelude::*;
        use std::sync::mpsc;
        use std::thread;

        let (sender, receiver) =
            mpsc::channel::<(PathId, Vec<paths::StepUpdate>)>();

        let mut paths = &mut self.paths;
        let mut nodes = &mut self.nodes;
        let mut occurrences = &mut self.occurrences;

        let mut mut_ctx = paths.get_multipath_mut_ctx();
        let refs_mut = mut_ctx.ref_muts_par();

        refs_mut.for_each_with(sender, |s, path| {
            let path_id = path.path_id;
            let updates = f(path_id, path);
            s.send((path_id, updates)).unwrap();
        });

        Self::apply_node_occurrence_consumer(receiver, nodes, occurrences);
    }
}
