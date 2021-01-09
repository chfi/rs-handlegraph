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
    AddDel, AddDelDelta, EdgesDelta, GraphOpDelta, LocalEdgeDelta, LocalStep,
    NodesDelta, PathsDelta, SinglePathDelta,
};
use ops::{CreateOp, GraphOp, GraphWideOp, MutHandleOp, MutPathOp, RemoveOp};
use traits::{DeriveDelta, GraphApply, GraphDelta};

impl CreateOp {
    pub fn derive_delta(&self, _graph: &PackedGraph) -> GraphOpDelta {
        let mut res = GraphOpDelta::default();
        match self {
            CreateOp::Handle { id, seq } => {
                let mut handles: AddDelDelta<Handle> = Default::default();
                handles.add(Handle::pack(*id, false));

                let nodes = NodesDelta {
                    node_count: 1,
                    total_len: seq.len() as isize,
                    handles,
                };
                res.nodes = nodes;
            }
            CreateOp::Edge { edge } => {
                let edges = EdgesDelta {
                    edge_count: 1,
                    new_edges: vec![*edge],
                    removed_edges: Vec::new(),
                    // TODO fix this
                    edge_deltas: Vec::new(),
                };
                res.edges = edges;
            }
            CreateOp::EdgesIter { edges } => {
                unimplemented!();
            }
            CreateOp::Path { name } => {
                unimplemented!();
            }
        }

        res
    }

    pub fn apply(&self, graph: &mut PackedGraph) {
        match self {
            CreateOp::Handle { id, seq } => {
                println!("adding id: {:?}", id);
                graph.create_handle(seq, *id);
            }
            CreateOp::Edge { edge } => {
                graph.create_edge(*edge);
            }
            CreateOp::EdgesIter { edges } => {
                // TODO
            }
            CreateOp::Path { name } => {
                graph.create_path(name, false);
                //
            }
        }
    }
}

impl RemoveOp {
    pub fn derive_delta(&self, graph: &PackedGraph) -> GraphOpDelta {
        let mut res = GraphOpDelta::default();

        match self {
            RemoveOp::Handle { handle } => {
                let handle = *handle;
                let seq_len = graph.node_len(handle) as isize;

                let mut handles: AddDelDelta<Handle> = Default::default();
                handles.del(handle);

                let nodes = NodesDelta {
                    node_count: -1,
                    total_len: -seq_len,
                    handles,
                };
                res.nodes = nodes;
            }
            RemoveOp::Edge { edge } => {
                let edges = EdgesDelta {
                    edge_count: -1,
                    new_edges: Vec::new(),
                    removed_edges: vec![*edge],
                    // TODO fix this
                    edge_deltas: Vec::new(),
                };
                res.edges = edges;
            }
            RemoveOp::Path { name } => {
                unimplemented!();
            }
        }

        res
    }

    pub fn apply(&self, graph: &mut PackedGraph) {
        match self {
            RemoveOp::Handle { handle } => {
                // println!("removing id: {:?}", handle.id());
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

impl GraphOpDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
        let nodes = self.nodes.compose(std::mem::take(&mut rhs.nodes));
        let edges = self.edges.compose(std::mem::take(&mut rhs.edges));
        let paths = self.paths.compose(std::mem::take(&mut rhs.paths));

        Self {
            nodes,
            edges,
            paths,
        }
    }

    pub fn compose_nodes(mut self, mut nodes: NodesDelta) -> Self {
        self.nodes = self.nodes.compose(nodes);
        self
    }

    pub fn compose_edges(mut self, mut edges: EdgesDelta) -> Self {
        self.edges = self.edges.compose(edges);
        self
    }

    pub fn compose_paths(mut self, mut paths: PathsDelta) -> Self {
        self.paths = self.paths.compose(paths);
        self
    }
}

pub struct DeltaEq {
    graph: PackedGraph,
    delta: GraphOpDelta,
}

impl DeltaEq {
    pub fn new(graph: &PackedGraph, delta: GraphOpDelta) -> Self {
        let graph = graph.clone();

        Self { graph, delta }
    }

    pub fn eq_delta(&self, other: &PackedGraph) -> bool {
        let expected_node_count =
            (self.graph.node_count() as isize) + self.delta.nodes.node_count;

        if other.node_count() as isize != expected_node_count {
            return false;
        }

        let expected_total_len =
            (self.graph.total_length() as isize) + self.delta.nodes.total_len;

        if other.total_length() as isize != expected_total_len {
            return false;
        }

        let expected_edge_count =
            (self.graph.edge_count() as isize) + self.delta.edges.edge_count;

        if other.edge_count() as isize != expected_edge_count {
            return false;
        }

        let expected_path_count =
            (self.graph.path_count() as isize) + self.delta.paths.path_count;

        if other.path_count() as isize != expected_path_count {
            return false;
        }

        // let expected_total_len =
        //     (self.graph.total_length() as isize) + self.delta.nodes.total_len;

        // if other.total_length() as isize != expected_total_len {
        //     return false;
        // }

        true
    }
}

/*
impl NodesDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
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
}
*/

impl LocalEdgeDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
        let new_left = std::mem::take(&mut self.new_left);
        let new_left = new_left
            .into_iter()
            .filter(|e| !rhs.removed_left.contains(e))
            .collect::<Vec<_>>();

        let new_right = std::mem::take(&mut self.new_right);
        let new_right = new_right
            .into_iter()
            .filter(|e| !rhs.removed_right.contains(e))
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

impl EdgesDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
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
}

impl SinglePathDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
        let step_count = self.step_count + rhs.step_count;
        let seq_len = self.seq_len + rhs.seq_len;

        let new_steps = std::mem::take(&mut self.new_steps);
        // TODO fix this
        // let new_steps = new_steps
        //     .into_iter()
        //     .filter(|e| rhs.removed_steps.iter().find()
        //     .collect::<Vec<_>>();

        let mut removed_steps = std::mem::take(&mut self.removed_steps);
        removed_steps.append(&mut rhs.removed_steps);
        removed_steps.sort();
        removed_steps.dedup();

        let new_head = rhs.new_head;
        let new_tail = rhs.new_tail;

        Self {
            step_count,
            seq_len,
            new_steps,
            removed_steps,
            new_head,
            new_tail,
        }
    }
}

impl PathsDelta {
    pub fn compose(mut self, mut rhs: Self) -> Self {
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
        handle: Handle::pack(10u64, false),
    };

    let delta_1 = op_1.derive_delta(&graph_1);
    let delta_eq_1 = DeltaEq::new(&graph_1, delta_1.clone());
    op_1.apply(&mut graph_1);

    println!("---------------------------");
    println!("  op 1");
    println!("{:#?}", delta_1);
    println!("compare: {}", delta_eq_1.eq_delta(&graph_1));
    println!();

    let delta_2 = op_2.derive_delta(&graph_1);
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
