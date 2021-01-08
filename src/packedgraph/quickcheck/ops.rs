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
    Handle { id: NodeId, seq: Vec<u8> },
    Edge { edge: Edge },
    EdgesIter { edges: Vec<Edge> },
    Path { name: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RemoveOp {
    Handle { handle: Handle },
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
