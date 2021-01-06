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

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodePropertiesDelta {
    pub node_count: isize,
    pub total_len: isize,
    pub new_handles: Vec<(Handle, Vec<u8>)>,
    pub removed_handles: Vec<Handle>,
}

impl NodePropertiesDelta {
    pub fn compose(mut self, rhs: Self) -> Self {
        let node_count = self.node_count + rhs.node_count;
        let total_len = self.total_len + rhs.total_len;

        let new_handles = std::mem::take(&mut self.new_handles);
        let new_handles = new_handles
            .into_iter()
            .filter(|(h, _)| rhs.removed_handles.contains(h))
            .collect::<Vec<_>>();

        let mut removed_handles: FnvHashSet<_> =
            self.removed_handles.into_iter().collect();
        removed_handles.extend(rhs.removed_handles.into_iter());

        let removed_handles: Vec<_> = removed_handles.into_iter().collect();

        Self {
            node_count,
            total_len,
            new_handles,
            removed_handles,
        }
    }
}

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

impl LocalEdgeDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
        let new_left = std::mem::take(&mut self.new_left);
        let new_left = new_left
            .into_iter()
            .filter(|e| rhs.removed_left.contains(e))
            .collect::<Vec<_>>();

        let new_right = std::mem::take(&mut self.new_right);
        let new_right = new_right
            .into_iter()
            .filter(|e| rhs.removed_right.contains(e))
            .collect::<Vec<_>>();

        let left_degree = self.left_degree + rhs.left_degree;
        let right_degree = self.right_degree + rhs.right_degree;

        let mut removed_left = std::mem::take(&mut self.removed_left);
        removed_left.append(&mut rhs.removed_left);
        removed_left.sort();
        removed_left.dedup();

        let mut removed_right = std::mem::take(&mut self.removed_right);
        removed_right.append(&mut rhs.removed_right);
        removed_right.sort();
        removed_right.dedup();

        Self {
            handle: self.handle,
            new_left,
            new_right,
            removed_left,
            removed_right,
            left_degree,
            right_degree,
        }
    }
}

pub struct EdgePropertiesDelta {
    pub edge_count: isize,
    pub new_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub edge_deltas: Vec<LocalEdgeDelta>,
}

impl EdgePropertiesDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
        let edge_count = self.edge_count + rhs.edge_count;

        let new_edges = std::mem::take(&mut self.new_edges);
        let new_edges = new_edges
            .into_iter()
            .filter(|e| rhs.removed_edges.contains(e))
            .collect::<Vec<_>>();

        let mut removed_edges = std::mem::take(&mut self.removed_edges);
        removed_edges.append(&mut rhs.removed_edges);
        removed_edges.sort();
        removed_edges.dedup();

        let mut edge_deltas = std::mem::take(&mut self.edge_deltas);
        edge_deltas.append(&mut rhs.edge_deltas);
        edge_deltas.sort();
        edge_deltas.dedup();

        Self {
            edge_count,
            new_edges,
            removed_edges,
            edge_deltas,
        }
    }
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
