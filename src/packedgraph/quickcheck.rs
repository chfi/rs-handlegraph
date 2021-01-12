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
    AddDel, AddDelDelta, DeltaEq, EdgesDelta, GraphOpDelta, NodesDelta,
    PathsDelta,
};
use ops::{CreateOp, GraphOp, GraphWideOp, MutHandleOp, MutPathOp, RemoveOp};
use traits::{DeriveDelta, GraphApply, GraphDelta};

use rand::prelude::*;

fn print_graph_ops(ops: &[GraphOp]) {
    println!(" - {} ops", ops.len());
    for (ix, op) in ops.iter().enumerate() {
        match op {
            GraphOp::Create { op } => match op {
                CreateOp::Handle { id, seq } => {}
                CreateOp::Edge { edge } => {
                    let Edge(from, to) = *edge;
                    println!(
                        "{:<3} - Create Edge - {:3} {:3}",
                        ix,
                        u64::from(from.id()),
                        u64::from(to.id())
                    );
                }
                CreateOp::EdgesIter { edges } => {}
                CreateOp::Path { name } => {}
            },
            GraphOp::Remove { op } => match op {
                RemoveOp::Handle { handle } => {}
                RemoveOp::Edge { edge } => {
                    let Edge(from, to) = *edge;
                    println!(
                        "{:<3} - Remove Edge - {:3} {:3}",
                        ix,
                        u64::from(from.id()),
                        u64::from(to.id())
                    );
                }
                RemoveOp::Path { name } => {}
            },
            GraphOp::MutHandle { op } => {}
            GraphOp::MutPath { op } => {}
            GraphOp::GraphWide { op } => {}
        }
    }
}

fn gen_edge_ops(edges: &[Edge], mut del_r: f64, shuffle: bool) -> Vec<GraphOp> {
    // `del_r` signifies to what extent edges will be removed and re-added;
    // 0.0 -> just add all edges
    // 1.0 -> all edges will be removed and re-added at least once
    del_r = del_r.max(0.0).min(1.0);

    // if `shuffle` is true, the edge create ops will be in random
    // order, if false, the same order as the provided `edges` slice

    let create_op = |edge: Edge| -> GraphOp {
        GraphOp::Create {
            op: CreateOp::Edge { edge },
        }
    };

    let remove_op = |edge: Edge| -> GraphOp {
        GraphOp::Remove {
            op: RemoveOp::Edge { edge },
        }
    };

    if del_r < f64::EPSILON {
        return edges.iter().map(|&edge| create_op(edge)).collect();
    }

    let mut rng = rand::thread_rng();

    // edges that will be removed and added back in
    let mut remove_add: FnvHashSet<Edge> = FnvHashSet::default();

    for &edge in edges.iter() {
        let v: f64 = rng.gen();
        if v <= del_r {
            remove_add.insert(edge);
        }
    }

    let edges: Vec<Edge> = if shuffle {
        edges
            .choose_multiple(&mut rng, edges.len())
            .copied()
            .collect()
    } else {
        edges.to_vec()
    };

    let mut remove: Vec<(usize, Edge)> = Vec::new();

    let mut ops = Vec::with_capacity(edges.len());

    for edge in edges {
        if remove_add.contains(&edge) {
            remove.push((ops.len(), edge));
        }
        ops.push(create_op(edge));
    }

    let mut readd: Vec<(usize, Edge)> = Vec::new();

    let mut count = 0usize;

    // `ix` is the index of the edge's create op, so we know to add the remove op somewhere after
    for &(ix, edge) in remove.iter() {
        let ix = ix + count;
        let rem_ix = rng.gen_range(ix + 1, ops.len() + 1);
        ops.insert(rem_ix, remove_op(edge));
        readd.push((rem_ix, edge));
        count += 1;
    }

    count = 0;

    for &(ix, edge) in readd.iter() {
        let ix = ix + count;
        let add_ix = rng.gen_range(ix + 1, ops.len() + 1);
        ops.insert(add_ix, create_op(edge));
        count += 1;
    }

    ops
}

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

#[test]
fn adding_edges_ops() {
    let orig_graph = crate::packedgraph::tests::test_graph_no_paths();

    let nodes: Vec<(NodeId, Vec<u8>)> = orig_graph
        .handles()
        .map(|h| {
            let id = h.id();
            let seq = orig_graph.sequence_vec(h);
            (id, seq)
        })
        .collect::<Vec<_>>();

    // let edges: Vec<Edge> = orig_graph.edges().map(|edge| {}).collect();

    let edges = orig_graph.edges().collect::<Vec<_>>();

    let mut graph = PackedGraph::new();

    for (id, seq) in nodes {
        graph.create_handle(&seq, id);
    }

    println!("edge count {}", edges.len());

    let edge_ops_zero = gen_edge_ops(&edges, 0.0, false);
    let edge_ops_mid = gen_edge_ops(&edges, 0.5, false);
    let edge_ops_one = gen_edge_ops(&edges, 1.0, false);
    let edge_ops_shuffle = gen_edge_ops(&edges, 0.5, true);

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.0 - no shuffle");
    print_graph_ops(&edge_ops_zero);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.5 - no shuffle");
    print_graph_ops(&edge_ops_mid);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 1.0 - no shuffle");
    print_graph_ops(&edge_ops_one);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.5 - shuffled");
    print_graph_ops(&edge_ops_shuffle);
    println!("-----------------------------------");

    let mut graph_zero = graph.clone();
    let mut graph_mid = graph.clone();
    let mut graph_one = graph.clone();
    let mut graph_shuffle = graph.clone();

    for op in edge_ops_zero {
        op.apply(&mut graph_zero);
    }

    for op in edge_ops_mid {
        op.apply(&mut graph_mid);
    }

    for op in edge_ops_one {
        op.apply(&mut graph_one);
    }

    for op in edge_ops_shuffle {
        op.apply(&mut graph_shuffle);
    }

    println!("expected edge count:      {}", orig_graph.edge_count());
    println!("graph_zero    edge count: {}", graph_zero.edge_count());
    println!("graph_mid     edge count: {}", graph_mid.edge_count());
    println!("graph_one     edge count: {}", graph_one.edge_count());
    println!("graph_shuffle edge count: {}", graph_shuffle.edge_count());

    let mut expected = orig_graph.edges().collect::<Vec<_>>();
    expected.sort();

    let mut edges_zero = graph_zero.edges().collect::<Vec<_>>();
    let mut edges_mid = graph_mid.edges().collect::<Vec<_>>();
    let mut edges_one = graph_one.edges().collect::<Vec<_>>();
    let mut edges_shuffle = graph_shuffle.edges().collect::<Vec<_>>();

    edges_zero.sort();
    edges_mid.sort();
    edges_one.sort();
    edges_shuffle.sort();

    let mut edges = edges;
    edges.sort();

    assert_eq!(expected, edges);
    assert_eq!(expected, edges_zero);
    assert_eq!(expected, edges_mid);
    assert_eq!(expected, edges_one);
    assert_eq!(expected, edges_shuffle);
}
