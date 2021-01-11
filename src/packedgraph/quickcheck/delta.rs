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

use fnv::{FnvHashMap, FnvHashSet};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GraphOpDelta {
    pub nodes: NodesDelta,
    pub edges: EdgesDelta,
    pub paths: PathsDelta,
    pub count: usize,
}

impl GraphOpDelta {
    pub fn nodes_iter(&self) -> std::slice::Iter<'_, AddDel<Handle>> {
        self.nodes.handles.iter()
    }

    pub fn edges_iter(&self) -> std::slice::Iter<'_, AddDel<Edge>> {
        self.edges.edges.iter()
    }

    pub fn paths_iter(&self) -> std::slice::Iter<'_, AddDel<PathId>> {
        self.paths.paths.iter()
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn compose_nodes(mut self, nodes: NodesDelta) -> Self {
        self.nodes = self.nodes.compose(nodes);
        self
    }

    pub fn compose_edges(mut self, edges: EdgesDelta) -> Self {
        self.edges = self.edges.compose(edges);
        self
    }

    pub fn compose_paths(mut self, paths: PathsDelta) -> Self {
        self.paths = self.paths.compose(paths);
        self
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodesDelta {
    pub node_count: isize,
    pub total_len: isize,
    pub handles: AddDelDelta<Handle>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgesDelta {
    pub edge_count: isize,
    pub edges: AddDelDelta<Edge>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PathsDelta {
    pub path_count: isize,
    pub total_steps: isize,
    pub paths: AddDelDelta<PathId>,
    pub path_steps: FnvHashMap<PathId, PathStepsDelta>,
}

/*
  Delta classifications
*/

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AddDelDelta<T: Sized + Copy> {
    vec: Vec<AddDel<T>>,
}

impl<T: Sized + Copy> Default for AddDelDelta<T> {
    fn default() -> Self {
        Self { vec: Vec::new() }
    }
}

impl<T: Sized + Copy> AddDelDelta<T> {
    pub fn new() -> Self {
        Self { vec: Vec::new() }
    }

    pub fn new_add(v: T, count: &mut usize) -> Self {
        let res = AddDelDelta {
            vec: vec![AddDel::Add(*count, v)],
        };
        *count += 1;
        res
    }

    pub fn new_del(v: T, count: &mut usize) -> Self {
        let res = AddDelDelta {
            vec: vec![AddDel::Del(*count, v)],
        };
        *count += 1;
        res
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, AddDel<T>> {
        self.vec.iter()
    }

    #[inline]
    pub fn add(&mut self, v: T, count: &mut usize) {
        self.vec.push(AddDel::Add(*count, v));
        *count += 1;
    }

    #[inline]
    pub fn del(&mut self, v: T, count: &mut usize) {
        self.vec.push(AddDel::Del(*count, v));
        *count += 1;
    }

    #[inline]
    pub fn append(&mut self, other: &Self) {
        // TODO think about the offsetting and how/if it should occur

        let offset = self.vec.last().map(|ad| ad.count()).unwrap_or(0);
        // let new_count = self.count + other.count;
        // let offset = self.count;

        self.vec.extend(
            other.vec.iter().copied().map(|ad| ad.offset_count(offset)),
        );
    }
}

impl<T> AddDelDelta<T>
where
    T: Sized + Copy + Eq + std::hash::Hash,
{
    #[inline]
    pub fn compact(&mut self) {
        let vec = std::mem::take(&mut self.vec);

        let mut parity: FnvHashMap<T, i8> = FnvHashMap::default();
        for &ad in vec.iter() {
            let diff = if ad.is_add() { 1 } else { -1 };
            *parity.entry(ad.value()).or_default() += diff;
        }

        let mut canonical: Vec<AddDel<T>> = Vec::with_capacity(vec.len());

        let mut seen: FnvHashSet<T> = FnvHashSet::default();

        for ad in vec.into_iter().rev() {
            let k = ad.value();

            if !seen.contains(&k) {
                seen.insert(k);

                if let Some(par) = parity.get(&k) {
                    match par.cmp(&0) {
                        std::cmp::Ordering::Less => {
                            canonical.push(AddDel::Del(ad.count(), k));
                        }
                        std::cmp::Ordering::Equal => {
                            // cancels out
                        }
                        std::cmp::Ordering::Greater => {
                            canonical.push(AddDel::Add(ad.count(), k));
                        }
                    }
                }
            }
        }
        canonical.reverse();
        canonical.shrink_to_fit();
        self.vec = canonical;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AddDel<T: Sized + Copy> {
    Add(usize, T),
    Del(usize, T),
}

impl<T: Sized + Copy> AddDel<T> {
    #[inline]
    pub fn add(&self, c: usize, v: T) -> Self {
        AddDel::Add(c, v)
    }

    #[inline]
    pub fn del(&self, c: usize, v: T) -> Self {
        AddDel::Del(c, v)
    }

    #[inline]
    pub fn is_add(&self) -> bool {
        match self {
            AddDel::Add(_, _) => true,
            AddDel::Del(_, _) => false,
        }
    }

    #[inline]
    pub fn is_del(&self) -> bool {
        match self {
            AddDel::Add(_, _) => false,
            AddDel::Del(_, _) => true,
        }
    }

    #[inline]
    pub fn count(&self) -> usize {
        match self {
            AddDel::Add(c, _) => *c,
            AddDel::Del(c, _) => *c,
        }
    }

    #[inline]
    pub fn value(&self) -> T {
        match self {
            AddDel::Add(_, v) => *v,
            AddDel::Del(_, v) => *v,
        }
    }

    #[inline]
    pub fn map<F, U>(&self, f: F) -> AddDel<U>
    where
        U: Sized + Copy,
        F: Fn(T) -> U,
    {
        match *self {
            AddDel::Add(c, t) => AddDel::Add(c, f(t)),
            AddDel::Del(c, t) => AddDel::Del(c, f(t)),
        }
    }

    #[inline]
    pub fn offset_count(&self, offset: usize) -> Self {
        match *self {
            AddDel::Add(c, t) => AddDel::Add(c + offset, t),
            AddDel::Del(c, t) => AddDel::Del(c + offset, t),
        }
    }
}

/*
  Trait impls
*/

impl GraphDelta for GraphOpDelta {
    fn compose(mut self, mut rhs: Self) -> Self {
        self.nodes = self.nodes.compose(std::mem::take(&mut rhs.nodes));
        self.edges = self.edges.compose(std::mem::take(&mut rhs.edges));
        self.paths = self.paths.compose(std::mem::take(&mut rhs.paths));
        self.count = rhs.count;
        self
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        self
    }
}

impl GraphDelta for NodesDelta {
    fn compose(mut self, rhs: Self) -> Self {
        let node_count = self.node_count + rhs.node_count;
        let total_len = self.total_len + rhs.total_len;

        println!("in nodes compose");
        let mut handles = std::mem::take(&mut self.handles);
        handles.append(&rhs.handles);
        handles.compact();

        Self {
            node_count,
            total_len,
            handles,
        }
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            nodes: self,
            ..GraphOpDelta::default()
        }
    }
}

impl GraphDelta for EdgesDelta {
    fn compose(mut self, rhs: Self) -> Self {
        let edge_count = self.edge_count + rhs.edge_count;

        let mut edges = std::mem::take(&mut self.edges);
        edges.append(&rhs.edges);
        edges.compact();

        Self { edge_count, edges }
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            edges: self,
            ..GraphOpDelta::default()
        }
    }
}

impl GraphDelta for PathsDelta {
    fn compose(mut self, mut rhs: Self) -> Self {
        let path_count = self.path_count + rhs.path_count;
        let total_steps = self.total_steps + rhs.total_steps;

        let mut paths = std::mem::take(&mut self.paths);
        paths.append(&rhs.paths);
        paths.compact();

        let mut path_steps = std::mem::take(&mut self.path_steps);

        for (path_id, lhs_steps) in path_steps.iter_mut() {
            if let Some(rhs_steps) = rhs.path_steps.get(path_id) {
                lhs_steps.step_count += rhs_steps.step_count;
                // lhs_steps.steps.append(&rhs_steps.steps);
                // lhs_steps.head = rhs_steps.head;
                // lhs_steps.tail = rhs_steps.tail;
            }
        }

        Self {
            path_count,
            total_steps,
            paths,
            path_steps,
        }
    }

    fn into_graph_delta(self) -> GraphOpDelta {
        GraphOpDelta {
            paths: self,
            ..GraphOpDelta::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathStepsDelta {
    pub path_id: PathId,
    pub step_count: isize,
    // pub steps: AddDelDelta<StepOp>,
    // pub head: StepPtr,
    // pub tail: StepPtr,
}

/*
   Delta-based invariant checking
*/

pub struct DeltaEq {
    graph: PackedGraph,
    delta: GraphOpDelta,
}

impl DeltaEq {
    pub fn new(graph: &PackedGraph, delta: GraphOpDelta) -> Self {
        let graph = graph.clone();

        Self { graph, delta }
    }

    fn compare_nodes(&self, other: &PackedGraph) -> bool {
        let expected_node_count =
            (self.graph.node_count() as isize) + self.delta.nodes.node_count;

        if other.node_count() as isize != expected_node_count {
            println!(
                "node count: {} != {}, delta {}",
                self.graph.node_count(),
                other.node_count(),
                self.delta.nodes.node_count
            );
            return false;
        }

        /*
        for handle_delta in self.delta.nodes_iter() {
            if handle_delta.is_add() {
                if !other.has_node(handle_delta.value().id()) {
                    return false;
                }
            } else {
                if other.has_node(handle_delta.value().id()) {
                    return false;
                }
            }
        }
        */

        let expected_total_len =
            (self.graph.total_length() as isize) + self.delta.nodes.total_len;

        if other.total_length() as isize != expected_total_len {
            println!(
                "total len: {} != {}, delta {}",
                self.graph.total_length(),
                other.total_length(),
                self.delta.nodes.total_len
            );
            return false;
        }

        true
    }

    fn compare_edges(&self, other: &PackedGraph) -> bool {
        let expected_edge_count =
            (self.graph.edge_count() as isize) + self.delta.edges.edge_count;

        if other.edge_count() as isize != expected_edge_count {
            println!("wrong edge count:");
            println!("  LHS: {}", self.graph.edge_count());
            println!("  RHS: {}", other.edge_count());
            println!("  edge delta:     {}", self.delta.edges.edge_count);
            return false;
        }

        /*
        for edge_delta in self.delta.edges.iter() {
            if edge_delta.is_add() {
                if !other.has_node(handle_delta.value().id()) {
                    return false;
                }
            } else {
                if other.has_node(handle_delta.value().id()) {
                    return false;
                }
            } else {
            }
        }
        */

        true
    }

    fn compare_paths(&self, other: &PackedGraph) -> bool {
        let expected_path_count =
            (self.graph.path_count() as isize) + self.delta.paths.path_count;

        if other.path_count() as isize != expected_path_count {
            println!("wrong path count:");
            println!("  LHS: {}", self.graph.path_count());
            println!("  RHS: {}", other.path_count());
            println!("  path delta:     {}", self.delta.paths.path_count);
            return false;
        }

        true
    }

    pub fn eq_delta(&self, other: &PackedGraph) -> bool {
        let mut pass = true;

        println!("  ------------------------  ");
        println!("      eq_delta");

        pass &= self.compare_nodes(other);

        pass &= self.compare_edges(other);

        pass &= self.compare_paths(other);

        println!("  ------------------------  ");

        pass
    }
}
