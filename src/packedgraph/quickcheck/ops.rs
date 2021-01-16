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
    defragment::Defragment,
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

use fnv::{FnvHashMap, FnvHashSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraphOp {
    Create { op: CreateOp },
    Remove { op: RemoveOp },
    MutHandle { op: MutHandleOp },
    MutPath { op: MutPathOp },
    // MutMultiPaths { ops: (PathId, MutPathOp) },
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
    InsertAfter {
        path: PathId,
        prev: StepPtr,
        handle: Handle,
    },
    RemoveAfter {
        path: PathId,
        prev: StepPtr,
    },
    InsertBefore {
        path: PathId,
        next: StepPtr,
        handle: Handle,
    },
    RemoveBefore {
        path: PathId,
        next: StepPtr,
    },
    FlipStep {
        path: PathId,
        step: StepPtr,
    },
    RewriteSegment {
        path: PathId,
        from: StepPtr,
        to: StepPtr,
        new: Vec<Handle>,
    },
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
  GraphOp trait impls
*/

impl DeriveDelta for GraphOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        match self {
            GraphOp::Create { op } => op.derive_compose(graph, lhs),
            GraphOp::Remove { op } => op.derive_compose(graph, lhs),
            GraphOp::MutHandle { op } => op.derive_compose(graph, lhs),
            GraphOp::MutPath { op } => op.derive_compose(graph, lhs),
            GraphOp::GraphWide { op } => op.derive_compose(graph, lhs),
        }
    }
}

impl GraphApply for GraphOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            GraphOp::Create { op } => op.apply(graph),
            GraphOp::Remove { op } => op.apply(graph),
            GraphOp::MutHandle { op } => op.apply(graph),
            GraphOp::MutPath { op } => op.apply(graph),
            GraphOp::GraphWide { op } => op.apply(graph),
        };
    }
}

/*
  CreateOp trait impls
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
                let path_id = PathId(graph.path_count() as u64);

                lhs.paths.path_count += 1;
                lhs.paths.paths.add(path_id, count);
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
  RemoveOp trait impls
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
                let path_id = graph.get_path_id(name).unwrap();

                lhs.paths.path_count -= 1;
                lhs.paths.paths.del(path_id, count);
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
  MutHandleOp trait impls
*/

impl DeriveDelta for MutHandleOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        mut lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        match self {
            MutHandleOp::Divide { handle, offsets } => {
                let node_len = graph.node_len(*handle);

                let (_, offsets_sum_len) = offsets.iter().copied().fold(
                    (0, 0),
                    |(last, sum), offset| {
                        let len = offset - last;
                        (offset, sum + len)
                    },
                );

                let new_count = if offsets_sum_len < node_len {
                    offsets.len() + 1
                } else {
                    offsets.len()
                };

                let mut next_id = u64::from(graph.max_node_id()) + 1;

                let mut handles: AddDelDelta<Handle> = Default::default();
                let mut edges: AddDelDelta<Edge> = Default::default();

                let mut node_count = 0isize;
                let mut edge_count = 0isize;

                let mut prev_h = *handle;

                let count = &mut lhs.count;

                for _ in 0..offsets.len() {
                    let curr_h = Handle::pack(next_id, false);

                    handles.add(curr_h, count);
                    edges.add(Edge(prev_h, curr_h), count);

                    prev_h = curr_h;
                    next_id += 1;

                    node_count += 1;
                    edge_count += 2;
                }

                lhs.nodes = NodesDelta {
                    node_count,
                    total_len: 0,
                    handles,
                };

                lhs.edges = EdgesDelta { edge_count, edges };

                lhs
            }
        }
    }
}

impl GraphApply for MutHandleOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            MutHandleOp::Divide { handle, offsets } => {
                graph.divide_handle(*handle, offsets);
            }
        }
    }
}

/*
  MutPathOp trait impls
*/

impl DeriveDelta for MutPathOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        mut lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        match self {
            MutPathOp::InsertAfter { path, prev, handle } => {
                // let path_steps: &mut FnvHashMap<PathId, PathStepsDelta> =
                //     &mut lhs.paths.path_steps;

                let path_steps: &mut PathStepsDelta =
                    lhs.paths.path_steps.get_mut(path).unwrap();

                path_steps.step_count += 1;

                lhs

                // TODO for now we just track step count, not the
                // specific path structure
                /*
                path_steps.steps.add(
                    StepOp::StepAfter {
                        prev: *prev,
                        handle: *handle,
                    },
                    &mut lhs.count,
                );


                if graph.path_last_step(*path) == Some(*prev) {
                    // TODO get the new step index and update the tail
                    // path_steps.tail =
                }
                */
            }
            MutPathOp::RemoveAfter { path, prev } => {
                let path_steps: &mut PathStepsDelta =
                    lhs.paths.path_steps.get_mut(path).unwrap();

                path_steps.step_count -= 1;

                lhs
            }
            MutPathOp::InsertBefore { path, next, handle } => {
                let path_steps: &mut PathStepsDelta =
                    lhs.paths.path_steps.get_mut(path).unwrap();

                path_steps.step_count += 1;

                lhs
                /*
                let prev = graph.path_steps.steps.add(
                    StepOp::InsertBefore {
                        next: *next,
                        handle: *handle,
                    },
                    &mut lhs.count,
                );

                if graph.path_first_step(*path) == Some(*next) {
                    // TODO get the new step index and update the head
                    // path_steps.head =
                }
                */
            }
            MutPathOp::RemoveBefore { path, next } => {
                let path_steps: &mut PathStepsDelta =
                    lhs.paths.path_steps.get_mut(path).unwrap();

                path_steps.step_count -= 1;

                lhs
            }
            MutPathOp::FlipStep { path, step } => lhs,
            MutPathOp::RewriteSegment {
                path,
                from,
                to,
                new,
            } => {
                let path_steps: &mut PathStepsDelta =
                    lhs.paths.path_steps.get_mut(path).unwrap();

                let added = new.len() as isize;
                // TODO calculate steps in range [from, to)
                let removed = 0;

                path_steps.step_count -= 1;

                lhs
            }
        }
    }
}

impl GraphApply for MutPathOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            MutPathOp::InsertAfter { path, prev, handle } => {
                graph.path_insert_step_after(*path, *prev, *handle);
            }
            MutPathOp::RemoveAfter { path, prev } => {
                let step = graph.path_next_step(*path, *prev).unwrap();
                graph.path_remove_step(*path, step);
            }
            MutPathOp::InsertBefore { path, next, handle } => {
                let step = graph.path_prev_step(*path, *next).unwrap();
                graph.path_insert_step_after(*path, step, *handle);
            }
            MutPathOp::RemoveBefore { path, next } => {
                let step = graph.path_prev_step(*path, *next).unwrap();
                graph.path_remove_step(*path, step);
            }
            MutPathOp::FlipStep { path, step } => {
                graph.path_flip_step(*path, *step);
            }
            MutPathOp::RewriteSegment {
                path,
                from,
                to,
                new,
            } => {
                graph.path_rewrite_segment(*path, *from, *to, new);
            }
        }
    }
}

/*
  GraphWideOp trait impls
*/

impl DeriveDelta for GraphWideOp {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        mut lhs: GraphOpDelta,
    ) -> GraphOpDelta {
        match self {
            GraphWideOp::Defragment => lhs,
            GraphWideOp::ApplyOrdering { order } => lhs,
        }
    }
}

impl GraphApply for GraphWideOp {
    fn apply(&self, graph: &mut PackedGraph) {
        match self {
            GraphWideOp::Defragment => {
                graph.defragment();
            }
            GraphWideOp::ApplyOrdering { order } => {
                graph.apply_ordering(order);
            }
        }
    }
}
