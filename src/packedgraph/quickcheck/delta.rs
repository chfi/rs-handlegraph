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
  Trait impls
*/

impl GraphDelta for GraphOpDelta {
    fn compose(self, mut rhs: Self) -> Self {
        let nodes = self.nodes.compose(std::mem::take(&mut rhs.nodes));
        let edges = self.edges.compose(std::mem::take(&mut rhs.edges));
        let paths = self.paths.compose(std::mem::take(&mut rhs.paths));

        Self {
            nodes,
            edges,
            paths,
        }
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        self
    }

    fn make_eq(&self, graph: &PackedGraph) -> DeltaEq {
        DeltaEq::new(graph, self.clone())
    }
}

impl GraphDelta for NodesDelta {
    fn compose(mut self, mut rhs: Self) -> Self {
        let node_count = self.node_count + rhs.node_count;
        let total_len = self.total_len + rhs.total_len;

        let new_handles = std::mem::take(&mut self.new_handles);
        let mut new_handles = new_handles
            .into_iter()
            .filter(|(h, _)| !rhs.removed_handles.contains(h))
            .collect::<Vec<_>>();
        new_handles.append(&mut rhs.new_handles);

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

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            nodes: self,
            ..GraphOpDelta::default()
        }
    }

    fn make_eq(&self, graph: &PackedGraph) -> DeltaEq {
        unimplemented!();
    }
}

impl GraphDelta for EdgesDelta {
    fn compose(mut self, mut rhs: Self) -> Self {
        let edge_count = self.edge_count + rhs.edge_count;

        let new_edges = std::mem::take(&mut self.new_edges);
        let new_edges = new_edges
            .into_iter()
            .filter(|e| !rhs.removed_edges.contains(e))
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

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            edges: self,
            ..GraphOpDelta::default()
        }
    }

    fn make_eq(&self, graph: &PackedGraph) -> DeltaEq {
        unimplemented!();
    }
}

impl GraphDelta for PathsDelta {
    fn compose(mut self, mut rhs: Self) -> Self {
        let path_count = self.path_count + rhs.path_count;
        let total_steps = self.total_steps + rhs.total_steps;

        let new_paths = std::mem::take(&mut self.new_paths);
        let new_paths = new_paths
            .into_iter()
            .filter(|e| !rhs.removed_paths.contains(e))
            .collect::<Vec<_>>();

        let mut removed_paths = std::mem::take(&mut self.removed_paths);
        removed_paths.append(&mut rhs.removed_paths);
        removed_paths.sort();
        removed_paths.dedup();

        Self {
            path_count,
            total_steps,
            new_paths,
            removed_paths,
        }
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            paths: self,
            ..GraphOpDelta::default()
        }
    }

    fn make_eq(&self, graph: &PackedGraph) -> DeltaEq {
        unimplemented!();
    }
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
