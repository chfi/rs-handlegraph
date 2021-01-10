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

/*
  CreateOp trait imps
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
                lhs.nodes = NodesDelta {
                    node_count: 1,
                    total_len: seq.len() as isize,
                    handles: AddDelDelta::new_add(
                        Handle::pack(*id, false),
                        count,
                    ),
                };
            }
            CreateOp::Edge { edge } => {
                lhs.edges = EdgesDelta {
                    edge_count: 1,
                    edges: AddDelDelta::new_add(*edge, count),
                };
            }
            CreateOp::EdgesIter { edges } => {
                let edge_count = edges.len() as isize;
                let edges =
                    edges.iter().fold(AddDelDelta::new(), |mut acc, &edge| {
                        acc.add(edge, count);
                        acc
                    });

                lhs.edges = EdgesDelta { edges, edge_count };
            }
            CreateOp::Path { name } => {
                unimplemented!();
            }
        }

        lhs
    }
}

impl GraphApply for CreateOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            CreateOp::Handle { id, seq } => {
                println!("adding id: {:?}", id);
                graph.create_handle(seq, *id);
            }
            CreateOp::Edge { edge } => {
                graph.create_edge(*edge);
            }
            CreateOp::EdgesIter { edges } => {
                graph.create_edges_iter(edges.iter().copied());
            }
            CreateOp::Path { name } => {
                graph.create_path(name, false);
            }
        }
    }
}

/*
  RemoveOp trait imps
*/

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

                lhs.nodes = NodesDelta {
                    node_count: -1,
                    total_len: -seq_len,
                    handles: AddDelDelta::new_del(handle, count),
                };

                let mut edges = AddDelDelta::new();
                let mut edge_count = 0isize;

                {
                    let mut add_edges = |a: Handle, b: Handle| {
                        edges.add(Edge(a, b), count);
                        edges.add(Edge(b.flip(), a.flip()), count);
                        edge_count -= 2;
                    };

                    graph
                        .neighbors(handle, Direction::Left)
                        .for_each(|l| add_edges(l, handle));
                    graph
                        .neighbors(handle, Direction::Right)
                        .for_each(|r| add_edges(handle, r));
                }

                lhs.edges = EdgesDelta { edges, edge_count };
            }
            RemoveOp::Edge { edge } => {
                lhs.edges = EdgesDelta {
                    edge_count: -1,
                    edges: AddDelDelta::new_del(*edge, count),
                };
            }
            RemoveOp::Path { name } => {
                unimplemented!();
            }
        }

        lhs
    }
}

impl GraphApply for RemoveOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            RemoveOp::Handle { handle } => {
                graph.remove_handle(*handle);
            }
            RemoveOp::Edge { edge } => {
                graph.remove_edge(*edge);
            }
            RemoveOp::Path { name } => {
                let path_id = graph.get_path_id(name).unwrap();
                graph.destroy_path(path_id);
            }
        }
    }
}

/*
  MutHandleOp trait imps
*/

/*
  MutPathOp trait imps
*/

/*
  GraphWideOp trait imps
*/
