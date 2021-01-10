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

use super::DeltaEq;
use super::GraphOpDelta;

pub trait GraphApply {
    fn apply(&self, graph: &mut PackedGraph);
}

pub trait DeriveDelta {
    fn derive_compose(
        &self,
        graph: &PackedGraph,
        lhs: GraphOpDelta,
    ) -> GraphOpDelta;

    fn derive_delta(&self, graph: &PackedGraph, count: usize) -> GraphOpDelta {
        let mut delta = GraphOpDelta::default();
        delta.count = count;
        self.derive_compose(graph, delta)
    }
}

pub trait GraphDelta: Sized + Clone {
    fn compose(self, rhs: Self) -> Self;

    fn into_graph_delta(self) -> GraphOpDelta;
}
