#![allow(dead_code)]
#[allow(unused_imports)]
use crate::{
    handle::{Edge, Handle, NodeId},
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

use crate::hashgraph::HashGraph;
use crate::packedgraph::PackedGraph;

fn hnd(x: u64) -> Handle {
    Handle::pack(x, false)
}

fn r_hnd(x: u64) -> Handle {
    Handle::pack(x, true)
}

fn vec_hnd(v: Vec<u64>) -> Vec<Handle> {
    v.into_iter().map(hnd).collect::<Vec<_>>()
}

fn edge(l: u64, r: u64) -> Edge {
    Edge(hnd(l), hnd(r))
}
fn r_edge(l: u64, r: u64) -> Edge {
    Edge(r_hnd(l), r_hnd(r))
}

pub fn test_hashgraph() -> HashGraph {
    test_graph_no_paths()
}

pub fn test_packedgraph() -> PackedGraph {
    test_graph_no_paths()
}

pub fn test_graph_no_paths<G>() -> G
where
    G: AdditiveHandleGraph + Default,
{
    use bstr::B;
    let mut graph = G::default();

    let seqs = vec![
        //                  Node
        B("GTCA"),       //  1
        B("AAGTGCTAGT"), //  2
        B("ATA"),        //  3
        B("AGTA"),       //  4
        B("GTCCA"),      //  5
        B("GGGT"),       //  6
        B("AACT"),       //  7
        B("AACAT"),      //  8
        B("AGCC"),       //  9
    ];
    /*
    1 ----- 8 --- 4 -----
      \   /   \     \     \
        2      \     \      6
      /   \     \     \   /
    5 ----- 7 --- 3 --- 9
    */

    let _handles = seqs
        .iter()
        .map(|seq| graph.append_handle(seq))
        .collect::<Vec<_>>();

    macro_rules! insert_edges {
            ($graph:ident, [$(($from:literal, $to:literal)),*]) => {
                $(
                    $graph.create_edge(edge($from, $to));
                )*
            };
        }

    insert_edges!(
        graph,
        [
            (1, 2),
            (1, 8),
            (5, 2),
            (5, 7),
            (2, 8),
            (2, 7),
            (7, 3),
            (8, 3),
            (8, 4),
            (3, 9),
            (4, 9),
            (4, 6),
            (9, 6)
        ]
    );

    graph
}
