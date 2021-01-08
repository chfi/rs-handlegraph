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
    fn derive_delta(&self, graph: &PackedGraph) -> GraphOpDelta;
}

pub trait GraphDelta: Sized + Clone {
    fn compose(self, rhs: Self) -> Self;

    fn into_graph_delta(self) -> GraphOpDelta;

    fn make_eq(&self, graph: &PackedGraph) -> DeltaEq;
}
