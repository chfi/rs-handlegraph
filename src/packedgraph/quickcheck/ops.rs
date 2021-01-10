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

use super::delta::*;
use super::traits::*;

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
    // Flip { handle: Handle },
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

/*
  Trait implementations
*/

impl DeriveDelta for CreateOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        mut lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        let count = &mut lhs.count;

        match self {
            CreateOp::Handle { id, seq } => {
                let mut handles: AddDelDelta<Handle> = AddDelDelta::new(*count);
                handles.add(Handle::pack(*id, false));
                *count += 1;

                let nodes = NodesDelta {
                    node_count: 1,
                    total_len: seq.len() as isize,
                    handles,
                };

                lhs.nodes = nodes;
            }
            CreateOp::Edge { edge } => {
                let mut edges: AddDelDelta<Edge> = AddDelDelta::new(*count);
                edges.add(*edge);
                *count += 1;

                let edges = EdgesDelta {
                    edge_count: 1,
                    edges,
                };
                lhs.edges = edges;
            }
            CreateOp::EdgesIter { edges } => {
                let mut edges_ad: AddDelDelta<Edge> = AddDelDelta::new(*count);
                let edge_count = edges.len() as isize;

                for &edge in edges {
                    edges_ad.add(edge);
                    *count += 1;
                }

                lhs.edges = EdgesDelta {
                    edges: edges_ad,
                    edge_count,
                };
            }
            CreateOp::Path { name } => {
                unimplemented!();
            }
        }

        lhs
    }
}

impl DeriveDelta for RemoveOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        mut lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        let count = &mut lhs.count;

        match self {
            RemoveOp::Handle { handle } => {
                let handle = *handle;
                let seq_len = graph.node_len(handle) as isize;

                let mut handles: AddDelDelta<Handle> = AddDelDelta::new(*count);
                handles.del(handle);
                *count += 1;

                lhs.nodes = NodesDelta {
                    node_count: -1,
                    total_len: -seq_len,
                    handles,
                };

                let mut edges: AddDelDelta<Edge> = AddDelDelta::new(*count);
                let mut edge_count = 0isize;

                for left in graph.neighbors(handle, Direction::Left) {
                    edges.add(Edge(left, handle));
                    edges.add(Edge(handle.flip(), left.flip()));
                    *count += 2;
                    edge_count -= 2;
                }
                for right in graph.neighbors(handle, Direction::Right) {
                    edges.add(Edge(handle, right));
                    edges.add(Edge(right.flip(), handle.flip()));
                    *count += 2;
                    edge_count -= 2;
                }

                lhs.edges = EdgesDelta { edges, edge_count };
            }
            RemoveOp::Edge { edge } => {
                let mut edges: AddDelDelta<Edge> = AddDelDelta::new(*count);
                edges.del(*edge);
                *count += 1;

                let edges = EdgesDelta {
                    edge_count: -1,
                    edges,
                };
                lhs.edges = edges;
            }
            RemoveOp::Path { name } => {
                unimplemented!();
            }
        }

        lhs
    }
}
