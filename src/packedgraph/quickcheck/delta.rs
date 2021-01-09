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

use crate::packedgraph::{
    edges::{EdgeListIx, EdgeLists},
    index::{list, OneBasedIndex, RecordIndex},
    iter::EdgeListHandleIter,
    nodes::IndexMapIter,
    occurrences::OccurrencesIter,
    paths::packedpath::StepPtr,
    sequence::DecodeIter,
    PackedGraph,
};

use super::traits::*;
use super::DeltaEq;

use fnv::{FnvHashMap, FnvHashSet};

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphOpDelta {
    pub nodes: NodesDelta,
    pub edges: EdgesDelta,
    pub paths: PathsDelta,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodesDelta {
    pub node_count: isize,
    pub total_len: isize,
    pub new_handles: Vec<(Handle, Vec<u8>)>,
    pub removed_handles: Vec<Handle>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgesDelta {
    pub edge_count: isize,
    pub new_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub edge_deltas: Vec<LocalEdgeDelta>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathsDelta {
    pub path_count: isize,
    pub total_steps: isize,
    pub new_paths: Vec<(PathId, Vec<u8>)>,
    pub removed_paths: Vec<(PathId, Vec<u8>)>,
}

/*
These may be scrapped in the future
*/

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalEdgeDelta {
    pub handle: Handle,
    pub new_left: Vec<Edge>,
    pub new_right: Vec<Edge>,
    pub removed_left: Vec<Edge>,
    pub removed_right: Vec<Edge>,
    pub left_degree: isize,
    pub right_degree: isize,
}

pub struct LocalStep {
    pub prev: (StepPtr, Handle),
    pub this: (StepPtr, Handle),
    pub next: (StepPtr, Handle),
}

pub struct SinglePathDelta {
    pub step_count: isize,
    pub seq_len: isize,
    pub new_steps: Vec<LocalStep>,
    pub removed_steps: Vec<StepPtr>,
    pub new_head: StepPtr,
    pub new_tail: StepPtr,
}
