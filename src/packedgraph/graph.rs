#[allow(unused_imports)]
pub(super) use super::{
    edges::{EdgeListIx, EdgeLists, EdgeVecIx},
    index::{NodeRecordId, OneBasedIndex, RecordIndex},
    nodes::{GraphVecIx, NodeIdIndexMap, NodeRecords},
    occurrences::{NodeOccurRecordIx, NodeOccurrences, OccurRecord},
    paths::{PackedGraphPaths, PathStepIx},
    sequence::{PackedSeqIter, SeqRecordIx, Sequences},
};

use crate::handle::{Handle, NodeId};

use crate::pathhandlegraph::PathId;

use crate::packed::traits::*;

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

    pub(super) fn add_node_occurrence(
        &mut self,
        handle: Handle,
        path_id: PathId,
        step: PathStepIx,
    ) {
        let rec_id = self.nodes.handle_record(handle).unwrap();
        let vec_ix = rec_id.to_zero_based().unwrap();

        let occur_ix = self.nodes.node_occurrence_map.get_unpack(vec_ix);

        let new_occur_ix =
            self.occurrences.append_entry(path_id, step, occur_ix);

        self.nodes
            .node_occurrence_map
            .set_pack(vec_ix, new_occur_ix);
    }

    pub(super) fn add_node_occurrences_iter<I>(
        &mut self,
        path_id: PathId,
        iter: I,
    ) where
        I: IntoIterator<Item = (Handle, PathStepIx)>,
    {
        iter.into_iter()
            .for_each(|(h, s)| self.add_node_occurrence(h, path_id, s))
    }
}
