#[allow(unused_imports)]
pub(super) use super::{
    edges::{EdgeListIx, EdgeLists, EdgeVecIx},
    index::{NodeRecordId, RecordIndex},
    nodes::{GraphVecIx, NodeIdIndexMap, NodeRecords},
    occurrences::{NodeOccurRecordIx, NodeOccurrences, OccurRecord},
    paths::{PackedGraphPaths, PathStepIx},
    sequence::{PackedSeqIter, SeqRecordIx, Sequences},
};

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
}
