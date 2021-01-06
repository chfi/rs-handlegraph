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

pub enum GraphOp {
    CreateHandle { id: Option<NodeId>, seq: Vec<u8> },
    CreateEdge { edge: Edge },
    CreateEdgesIter { edges: Vec<Edge> },
    CreatePath { name: Vec<u8> },
    RemoveHandle { id: NodeId },
    RemoveEdge { edge: Edge },
    RemovePath { name: Vec<u8> },
    FlipHandle { handle: Handle },
    DivideHandle { handle: Handle, offsets: Vec<usize> },
    AppendStep { path: PathId, handle: Handle },
    PrependStep { path: PathId, handle: Handle },
    FlipStep { path: PathId, handle: Handle },
    RewriteSegment { path: PathId, handle: Handle },
    Defragment,
    ApplyOrdering { order: Vec<Handle> },
}
