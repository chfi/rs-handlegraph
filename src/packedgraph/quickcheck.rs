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

use crate::hashgraph::HashGraph;

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
                        // "{:<3} - Create Edge - {:3} {:3}",
                        "{:<3} - Add1 - ({:2}, {:2})",
                        ix,
                        u64::from(from.0),
                        u64::from(to.0)
                    );
                }
                CreateOp::EdgesIter { edges } => {
                    // print!("{:<3} - Edges Iter  - ", ix);
                    print!("{:<3} - AddN - ", ix);
                    for (ix, edge) in edges.iter().enumerate() {
                        if ix != 0 {
                            print!(", ");
                        }

                        let Edge(from, to) = *edge;
                        print!(
                            "({:2}, {:2})",
                            u64::from(from.0),
                            u64::from(to.0)
                        );
                    }
                    println!();
                }
                CreateOp::Path { name } => {}
            },
            GraphOp::Remove { op } => match op {
                RemoveOp::Handle { handle } => {}
                RemoveOp::Edge { edge } => {
                    let Edge(from, to) = *edge;

                    println!(
                        "{:<3} - Del1 - ({:2}, {:2})",
                        // "{:<3} - Remove Edge - {:3} {:3}",
                        ix,
                        u64::from(from.0),
                        u64::from(to.0)
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

/// Takes a sequence of [`GraphOp`]s and batches sequences of
/// `CreateOp::Edge` into equivalent `CreateOp::EdgeIter` ops.
// fn batch_edge_ops(ops: &[GraphOp], mut batch_freq: f64) -> Vec<GraphOp> {
fn batch_edge_ops(ops: &[GraphOp]) -> Vec<GraphOp> {
    let mut batch_ops = Vec::with_capacity(ops.len());

    let mut latest_batch: Vec<Edge> = Vec::new();

    for op in ops {
        if let GraphOp::Create { op } = op {
            if let CreateOp::Edge { edge } = op {
                latest_batch.push(*edge);
            } else {
                batch_ops.push(GraphOp::Create { op: op.clone() });
            }
        } else {
            if !latest_batch.is_empty() {
                let edges = std::mem::take(&mut latest_batch);
                let batched = CreateOp::EdgesIter { edges };
                batch_ops.push(GraphOp::Create { op: batched });
            }
            batch_ops.push(op.clone());
        }
    }

    if !latest_batch.is_empty() {
        let edges = std::mem::take(&mut latest_batch);
        let batched = CreateOp::EdgesIter { edges };
        batch_ops.push(GraphOp::Create { op: batched });
    }

    batch_ops.shrink_to_fit();
    batch_ops
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

    let mut edges = edges.to_vec();

    if shuffle {
        edges.shuffle(&mut rng);
    }

    /*
    let edges: Vec<Edge> = if shuffle {
        edges
            .choose_multiple(&mut rng, edges.len())
            .copied()
            .collect()
    } else {
        edges.to_vec()
    };
    */

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
fn create_edges_iter_eq() {
    let orig_graph = crate::packedgraph::tests::test_graph_no_paths();

    let nodes: Vec<(NodeId, Vec<u8>)> = orig_graph
        .handles()
        .map(|h| {
            let id = h.id();
            let seq = orig_graph.sequence_vec(h);
            (id, seq)
        })
        .collect::<Vec<_>>();

    let edges = orig_graph.edges().collect::<Vec<_>>();

    let mut graph_simple = PackedGraph::new();
    let mut graph_batch = PackedGraph::new();

    for (id, seq) in nodes {
        graph_simple.create_handle(&seq, id);
        graph_batch.create_handle(&seq, id);
    }

    // let del_r = 0.3;
    let del_r = 0.0;
    let shuffle = false;

    let ops_simple = gen_edge_ops(&edges, del_r, shuffle);
    let ops_batch = batch_edge_ops(&ops_simple);

    println!("---------------------");
    println!("  --- simple ops --- ");
    print_graph_ops(&ops_simple);
    println!();
    println!("  ---  batch ops --- ");
    print_graph_ops(&ops_batch);
    println!("---------------------");

    for op in ops_simple {
        op.apply(&mut graph_simple);
    }

    for op in ops_batch {
        op.apply(&mut graph_batch);
    }

    let mut simple_edges = graph_simple.edges().collect::<Vec<_>>();
    let mut batch_edges = graph_batch.edges().collect::<Vec<_>>();

    simple_edges.sort();
    batch_edges.sort();

    assert_eq!(simple_edges, batch_edges);

    println!("   --- simple ---");
    println!("  node count: {}", graph_simple.node_count());
    println!("  edge count: {}", graph_simple.edge_count());
    println!("  edge records:");
    crate::packedgraph::tests::print_edge_records(&graph_simple);
    println!();

    println!("   --- batch  ---");
    println!("  node count: {}", graph_batch.node_count());
    println!("  edge count: {}", graph_batch.edge_count());
    println!("  edge records:");
    crate::packedgraph::tests::print_edge_records(&graph_batch);
    println!();
}

#[test]
fn graph_edges_ops() {
    let packed = crate::util::test::test_packedgraph();
    let hash = crate::util::test::test_hashgraph();

    let mut p_edges = packed.edges().collect::<Vec<_>>();
    let mut h_edges = hash.edges().collect::<Vec<_>>();

    p_edges.sort();
    h_edges.sort();

    println!(
        "{:2} | {:3} {:3} | {:3} {:3}",
        "Ix", "H_l", "H_r", "I_l", "I_r"
    );
    for (ix, &p_edge) in p_edges.iter().enumerate() {
        let Edge(p_f, p_t) = p_edge;

        println!(
            "{:2} | {:3} {:3} | {:3} {:3}",
            ix,
            p_f.0,
            p_t.0,
            p_f.id().0,
            p_t.id().0,
        );
    }

    let nodes: Vec<(NodeId, Vec<u8>)> = packed
        .handles()
        .map(|h| {
            let id = h.id();
            let seq = packed.sequence_vec(h);
            (id, seq)
        })
        .collect::<Vec<_>>();

    // let edges: Vec<Edge> = orig_graph.edges().map(|edge| {}).collect();

    let mut hash_2 = HashGraph::default();
    let mut packed_2 = PackedGraph::default();

    for (id, seq) in nodes {
        hash_2.create_handle(&seq, id);
        packed_2.create_handle(&seq, id);
    }

    for &edge in h_edges.iter() {
        hash_2.create_edge(edge);
    }

    for &edge in p_edges.iter() {
        packed_2.create_edge(edge);
    }

    let mut h2_edges = hash_2.edges().collect::<Vec<_>>();
    h2_edges.sort();

    let mut p2_edges = packed_2.edges().collect::<Vec<_>>();
    p2_edges.sort();

    assert_eq!(h_edges, h2_edges);
    assert_eq!(p_edges, p2_edges);

    // println!("{:2} | {:6}  {:6}", "Ix", "Packed", "Hash");
    // for (ix, (&p_edge, &h_edge)) in
    //     p_edges.iter().zip(h_edges.iter()).enumerate()
    // {
    //     let Edge(p_f, p_t) = p_edge;
    //     let Edge(h_f, h_t) = h_edge;

    //     println!(
    //         "{:2} |  {:>2}-{:<2}  {:>2}-{:<2}",
    //         ix, p_f.0, p_t.0, h_f.0, h_t.0
    //     );
    // }

    /*
    assert_eq!(p_edges, h_edges);

    let mut p_handles = packed.handles().collect::<Vec<_>>();
    let mut h_handles = hash.handles().collect::<Vec<_>>();

    p_handles.sort();
    h_handles.sort();

    assert_eq!(p_handles, h_handles);
    */

    // println!("-------------------");
    // println!(" PackedGraph edges ");
    // println!("  count: {}", p_edges.len());
    // println!("{:#?}", p_edges);
    // println!();

    // println!("-------------------");
    // println!("   HashGraph edges ");
    // println!("  count: {}", h_edges.len());
    // println!("{:#?}", h_edges);
    // println!();
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

    let batched_zero = batch_edge_ops(&edge_ops_zero);
    let batched_mid = batch_edge_ops(&edge_ops_mid);
    let batched_one = batch_edge_ops(&edge_ops_one);
    let batched_shuffle = batch_edge_ops(&edge_ops_shuffle);

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.0 - no shuffle");
    print_graph_ops(&edge_ops_zero);
    println!("    ---------------------------");
    println!("  Batched ");
    print_graph_ops(&batched_zero);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.5 - no shuffle");
    print_graph_ops(&edge_ops_mid);
    println!("    ---------------------------");
    println!("  Batched ");
    print_graph_ops(&batched_mid);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 1.0 - no shuffle");
    print_graph_ops(&edge_ops_one);
    println!("    ---------------------------");
    println!("  Batched ");
    print_graph_ops(&batched_one);
    println!("-----------------------------------");

    println!("-----------------------------------");
    println!("  Edge Ops - del_r 0.5 - shuffled");
    print_graph_ops(&edge_ops_shuffle);
    println!("    ---------------------------");
    println!("  Batched ");
    print_graph_ops(&batched_shuffle);
    println!("-----------------------------------");

    let mut graph_zero = graph.clone();
    let mut graph_mid = graph.clone();
    let mut graph_one = graph.clone();
    let mut graph_shuffle = graph.clone();

    let mut graph_zero_batched = graph.clone();
    let mut graph_mid_batched = graph.clone();
    let mut graph_one_batched = graph.clone();
    let mut graph_shuffle_batched = graph.clone();

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

    for op in batched_zero {
        op.apply(&mut graph_zero_batched);
    }

    for op in batched_mid {
        op.apply(&mut graph_mid_batched);
    }

    for op in batched_one {
        op.apply(&mut graph_one_batched);
    }

    for op in batched_shuffle {
        op.apply(&mut graph_shuffle_batched);
    }

    println!("expected edge count:      {}", orig_graph.edge_count());
    println!("graph_zero    edge count: {}", graph_zero.edge_count());
    println!("graph_mid     edge count: {}", graph_mid.edge_count());
    println!("graph_one     edge count: {}", graph_one.edge_count());
    println!("graph_shuffle edge count: {}", graph_shuffle.edge_count());

    println!(
        "graph_zero_batched    edge count: {}",
        graph_zero_batched.edge_count()
    );
    println!(
        "graph_mid_batched     edge count: {}",
        graph_mid_batched.edge_count()
    );

    println!(
        "graph_one_batched     edge count: {}",
        graph_one_batched.edge_count()
    );
    println!(
        "graph_shuffle_batched edge count: {}",
        graph_shuffle_batched.edge_count()
    );

    let mut expected = orig_graph.edges().collect::<Vec<_>>();
    expected.sort();

    /*
    for (ix, exp_edge) in expected.into_iter().enumerate() {
        let Edge(l, r) = exp_edge;
        println!(
            "{:2} - {} {} - {}",
            ix,
            l.id().0,
            r.id().0,
            graph_zero.has_edge(l, r)
        );
        // assert!(graph_zero.has_edge(l, r));
    }
    */

    let mut expected = orig_graph.edges().collect::<Vec<_>>();
    expected.sort();

    let mut edges_zero = graph_zero.edges().collect::<Vec<_>>();
    let mut edges_mid = graph_mid.edges().collect::<Vec<_>>();
    let mut edges_one = graph_one.edges().collect::<Vec<_>>();
    let mut edges_shuffle = graph_shuffle.edges().collect::<Vec<_>>();

    let mut edges_zero_batched = graph_zero_batched.edges().collect::<Vec<_>>();
    let mut edges_mid_batched = graph_mid_batched.edges().collect::<Vec<_>>();
    let mut edges_one_batched = graph_one_batched.edges().collect::<Vec<_>>();
    let mut edges_shuffle_batched =
        graph_shuffle_batched.edges().collect::<Vec<_>>();

    let print_edges = |title: &str, edges: &[Edge]| {
        println!("  -- {:^16} --", title);
        println!("| {:2} | {:^3} | {:^3} |", "Ix", "L", "R",);
        for (ix, &p_edge) in edges.iter().enumerate() {
            let Edge(p_f, p_t) = p_edge;
            println!("| {:2} | {:^3} | {:^3} |", ix, p_f.0, p_t.0,);
        }
        println!();
    };

    print_edges("zero simple", &edges_zero);
    print_edges("zero batch", &edges_zero_batched);

    print_edges("mid simple", &edges_mid);
    print_edges("mid batch", &edges_mid_batched);

    print_edges("one simple", &edges_one);
    print_edges("one batch", &edges_one_batched);

    let print_removed = |title: &str, graph: &PackedGraph| {
        print!(
            " - {:<22} has {:2} removed edge records: ",
            title,
            graph.edges.removed_records.len()
        );
        for (ix, edge_ix) in graph.edges.removed_records.iter().enumerate() {
            if ix != 0 {
                print!(", ");
            }
            print!("{}", edge_ix.to_vector_value());
        }
        println!();
    };

    println!();

    print_removed("graph_zero", &graph_zero);
    print_removed("graph_mid", &graph_mid);
    print_removed("graph_one", &graph_one);
    print_removed("graph_shuffle", &graph_shuffle);

    println!();

    print_removed("graph_zero_batched", &graph_zero_batched);
    print_removed("graph_mid_batched", &graph_mid_batched);
    print_removed("graph_one_batched", &graph_one_batched);
    print_removed("graph_shuffle_batched", &graph_shuffle_batched);

    edges_zero.sort();
    edges_mid.sort();
    edges_one.sort();
    edges_shuffle.sort();

    edges_zero_batched.sort();
    edges_mid_batched.sort();
    edges_one_batched.sort();
    edges_shuffle_batched.sort();

    println!("graph_mid");
    crate::packedgraph::tests::print_edge_records(&graph_mid);
    println!();

    println!("graph_one");
    crate::packedgraph::tests::print_edge_records(&graph_one);
    println!();

    println!("graph_shuffle");
    crate::packedgraph::tests::print_edge_records(&graph_shuffle);
    println!();

    println!("graph_mid_batched");
    crate::packedgraph::tests::print_edge_records(&graph_mid_batched);
    println!();

    println!("graph_one_batched");
    crate::packedgraph::tests::print_edge_records(&graph_one_batched);
    println!();

    println!("graph_shuffle_batched");
    crate::packedgraph::tests::print_edge_records(&graph_shuffle_batched);
    println!();

    // assert_eq!(edges_zero, edges_zero_batched);
    // assert_eq!(edges_mid, edges_mid_batched);
    // assert_eq!(edges_one, edges_one_batched);
    // assert_eq!(edges_shuffle, edges_shuffle_batched);

    /*
    let mut edges = edges;
    edges.sort();

    assert_eq!(expected, edges);
    assert_eq!(expected, edges_zero);
    assert_eq!(expected, edges_mid);
    assert_eq!(expected, edges_one);
    assert_eq!(expected, edges_shuffle);
    */
}
