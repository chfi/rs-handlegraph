#![allow(dead_code)]
#![allow(unused_imports)]

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

use super::{
    edges::{EdgeListIx, EdgeLists},
    index::{list, OneBasedIndex, RecordIndex},
    iter::EdgeListHandleIter,
    nodes::IndexMapIter,
    occurrences::OccurrencesIter,
    paths::packedpath::StepPtr,
    sequence::DecodeIter,
    PackedGraph,
};

use quickcheck::{Arbitrary, Gen, QuickCheck};

use fnv::{FnvHashMap, FnvHashSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphOp {
    Create { op: CreateOp },
    Remove { op: RemoveOp },
    MutHandle { op: MutHandleOp },
    MutPath { path: PathId, op: MutPathOp },
    MutMultiPaths { ops: (PathId, MutPathOp) },
    GraphWide { op: GraphWideOp },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CreateOp {
    Handle { id: Option<NodeId>, seq: Vec<u8> },
    Edge { edge: Edge },
    EdgesIter { edges: Vec<Edge> },
    Path { name: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RemoveOp {
    Handle { id: NodeId },
    Edge { edge: Edge },
    Path { name: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MutHandleOp {
    Flip { handle: Handle },
    Divide { handle: Handle, offsets: Vec<usize> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MutPathOp {
    AppendStep { handle: Handle },
    PrependStep { handle: Handle },
    FlipStep { handle: Handle },
    RewriteSegment { handle: Handle },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphWideOp {
    Defragment,
    ApplyOrdering { order: Vec<Handle> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodePropertiesDelta {
    pub node_count: isize,
    pub total_len: isize,
    pub max_id: NodeId,
    pub new_handles: Vec<(Handle, Vec<u8>)>,
    pub removed_handles: Vec<Handle>,
}

pub struct LocalEdgeDelta {
    pub handle: Handle,
    pub new_left: Vec<Edge>,
    pub new_right: Vec<Edge>,
    pub removed_left: Vec<Edge>,
    pub removed_right: Vec<Edge>,
    pub left_degree: isize,
    pub right_degree: isize,
}

pub struct EdgePropertiesDelta {
    pub edge_count: isize,
    pub new_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub edge_deltas: Vec<LocalEdgeDelta>,
}

pub struct PathPropertiesDelta {
    pub path_count: isize,
    pub total_steps: isize,
    pub new_paths: Vec<(PathId, Vec<u8>)>,
    pub removed_paths: Vec<(PathId, Vec<u8>)>,
}

pub struct LocalStep {
    pub prev: (StepPtr, Handle),
    pub this: (StepPtr, Handle),
    pub next: (StepPtr, Handle),
}

pub struct SinglePathPropertiesDelta {
    pub step_count: isize,
    pub seq_len: isize,
    pub new_steps: Vec<LocalStep>,
    pub removed_steps: Vec<StepPtr>,
    pub new_head: StepPtr,
    pub new_tail: StepPtr,
}
