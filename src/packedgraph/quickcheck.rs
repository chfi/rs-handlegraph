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

mod delta;
mod ops;
mod traits;

pub use delta::{
    AddDel, AddDelDelta, DeltaEq, EdgesDelta, GraphOpDelta, LocalStep,
    NodesDelta, PathsDelta,
};
use ops::{CreateOp, GraphOp, GraphWideOp, MutHandleOp, MutPathOp, RemoveOp};
use traits::{DeriveDelta, GraphApply, GraphDelta};

/*
impl MutHandleOp {
    pub fn derive_delta(&self, graph: &PackedGraph) -> GraphOpDelta {
        match self {
            MutHandleOp::Divide { handle, offsets } => {
                let mut delta = GraphOpDelta::default();

                let mut next_id = u64::from(graph.max_node_id()) + 1;

                let mut handles: AddDelDelta<Handle> = Default::default();
                let mut edges: AddDelDelta<Edge> = Default::default();

                let node_count = offsets.len() as isize;
                let edge_count = offsets.len() as isize;

                let mut prev_handle = handle;

                for _ in offsets {
                    let new_handle = Handle::pack(next_id, handle.is_reverse());
                    handles.add(new_handle);
                    next_id += 1;
                }

                delta.nodes.node_count = node_count;
                delta.nodes.handles = handles;

                delta.edges.edge_count = edge_count;
                delta.edges.edges = edges;

                delta
            }
        }
    }

    pub fn apply(&self, graph: &mut PackedGraph) {
        match self {
            MutHandleOp::Divide { handle, offsets } => {
                graph.divide_handle(*handle, offsets.to_owned());
            }
        }
    }
}
*/

#[test]
fn adding_nodes_prop() {
    let mut graph_1 = crate::packedgraph::tests::test_graph_no_paths();
    let mut graph_2 = crate::packedgraph::tests::test_graph_no_paths();

    let op_1 = CreateOp::Handle {
        id: 10u64.into(),
        seq: vec![b'A', b'G', b'G', b'T', b'C'],
    };

    // let op_2 = CreateOp::Handle {
    //     id: 11u64.into(),
    //     seq: vec![b'A', b'A', b'A'],
    // };

    let op_2 = RemoveOp::Handle {
        handle: Handle::pack(8u64, false),
    };

    // let mut count
    let count = 0usize;

    let delta_1 = op_1.derive_delta(&graph_1, count);
    let count = delta_1.count;
    let delta_eq_1 = DeltaEq::new(&graph_1, delta_1.clone());
    op_1.apply(&mut graph_1);

    println!("---------------------------");
    println!("  op 1");
    println!("{:#?}", delta_1);
    println!("compare: {}", delta_eq_1.eq_delta(&graph_1));
    println!();

    let delta_2 = op_2.derive_delta(&graph_1, count);
    let count = delta_2.count;
    let delta_eq_2 = DeltaEq::new(&graph_1, delta_2.clone());
    op_2.apply(&mut graph_1);

    println!("---------------------------");
    println!("  op 2");
    println!("{:#?}", delta_2);
    println!("compare: {}", delta_eq_2.eq_delta(&graph_1));
    println!();

    let delta_compose = delta_1.compose(delta_2);
    let comp_eq = DeltaEq::new(&graph_2, delta_compose.clone());
    println!("---------------------------");
    println!("  composed ops");
    println!("{:#?}", delta_compose);
    println!("compare to new:  {}", comp_eq.eq_delta(&graph_1));
    println!("compare to orig: {}", comp_eq.eq_delta(&graph_2));
    println!();
}
