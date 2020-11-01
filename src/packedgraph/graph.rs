pub(crate) static NARROW_PAGE_WIDTH: usize = 256;
pub(crate) static WIDE_PAGE_WIDTH: usize = 1024;

pub use super::{
    edges::{EdgeListIter, EdgeListIx, EdgeLists, EdgeVecIx},
    nodes::{GraphRecordIx, GraphVecIx, NodeIdIndexMap, NodeRecords},
    sequence::{PackedSeqIter, SeqRecordIx, Sequences},
};

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
}
