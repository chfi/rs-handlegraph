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
    pub handles: AddDelDelta<Handle>,
    // pub new_handles: Vec<(Handle, Vec<u8>)>,
    // pub removed_handles: Vec<Handle>,
    // pub handles: Vec<AddDel<Handle>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgesDelta {
    pub edge_count: isize,
    pub new_edges: Vec<Edge>,
    pub removed_edges: Vec<Edge>,
    pub edge_deltas: Vec<LocalEdgeDelta>,
    // pub edges: Vec<AddDel<Edge>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathsDelta {
    pub path_count: isize,
    pub total_steps: isize,
    pub new_paths: Vec<(PathId, Vec<u8>)>,
    pub removed_paths: Vec<(PathId, Vec<u8>)>,
}

/*
  Delta classifications
*/

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AddDelDelta<T: Sized + Copy> {
    vec: Vec<AddDel<T>>,
    count: usize,
}

impl<T: Sized + Copy> Default for AddDelDelta<T> {
    fn default() -> Self {
        Self {
            vec: Vec::new(),
            count: 0,
        }
    }
}

impl<T: Sized + Copy> AddDelDelta<T> {
    #[inline]
    pub fn add(&mut self, v: T) {
        self.vec.push(AddDel::Add(self.count, v));
        self.count += 1;
    }

    #[inline]
    pub fn del(&mut self, v: T) {
        self.vec.push(AddDel::Del(self.count, v));
        self.count += 1;
    }

    #[inline]
    pub fn append(&mut self, other: &Self) {
        let new_count = self.count + other.count;
        let offset = self.count;

        self.vec.extend(
            other.vec.iter().copied().map(|ad| ad.offset_count(offset)),
        );

        self.count = new_count;
    }
}

impl<T> AddDelDelta<T>
where
    T: Sized + Copy + Eq + std::hash::Hash,
{
    #[inline]
    pub fn compact(&mut self) {
        let vec = std::mem::take(&mut self.vec);

        // let mut seen: FnvHashSet<(bool, T)> = FnvHashSet::default();
        let mut seen: FnvHashSet<T> = FnvHashSet::default();
        seen.reserve(vec.len());

        // this contains the temporally last occurrence for each
        // element
        let mut canonical: Vec<AddDel<T>> = vec
            .into_iter()
            .rev()
            .filter(|ad| {
                let v = ad.value();
                if seen.contains(&v) {
                    false
                } else {
                    seen.insert(v);
                    true
                }
            })
            .collect();

        canonical.reverse();
        canonical.shrink_to_fit();
        self.vec = canonical;

        // let mut value_ops: FnvHashMap<T, Vec<AddDel<()>>> =
        //     FnvHashMap::default();
        // value_ops.reserve(vec.len());

        // for &ad in vec.iter() {
        //     let t = ad.value();
        //     value_ops.entry(t).or_default().push(ad.map(|_| ()));
        // }

        /*
        let mut seen: FnvHashSet<(bool, T)> = FnvHashSet::default();
        seen.reserve(vec.len());

        let mut seen_adds: FnvHashSet<T> = FnvHashSet::default();
        let mut seen_dels: FnvHashSet<T> = FnvHashSet::default();

        let mut canonical_adds: Vec<AddDel<T>> = Vec::with_capacity(vec.len());
        let mut canonical_dels: Vec<AddDel<T>> = Vec::with_capacity(vec.len());

        for ad in vec.into_iter().rev() {
            if ad.is_add() {
                if !seen_adds.contains(&ad.value()) {
                    seen_adds.insert(ad.value());
                    canonical_adds.push(ad);
                }
            } else {
                if !seen_dels.contains(&ad.value()) {
                    seen_dels.insert(ad.value());
                    canonical_dels.push(ad);
                }
            }
        }

        canonical_adds.reverse();
        canonical_dels.reverse();
        */

        /*
        let (mut adds, mut dels): (Vec<_>, Vec<_>) =
            vec.into_iter().partition(AddDel::is_add);

        let mut modified: FnvHashSet<_> = {
            let adds_set: FnvHashSet<_> = adds.iter().collect();
            let dels_set: FnvHashSet<_> = dels.iter().collect();

            let intersection = adds_set.intersection(&dels_set);
            intersection.cloned().collect()
        };
            */
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AddDel<T: Sized + Copy> {
    Add(usize, T),
    Del(usize, T),
}

impl<T: Sized + Copy> AddDel<T> {
    #[inline]
    pub fn add_init(v: T) -> Self {
        AddDel::Add(0, v)
    }

    #[inline]
    pub fn del_init(v: T) -> Self {
        AddDel::Del(0, v)
    }

    #[inline]
    pub fn add(&self, v: T) -> Self {
        AddDel::Add(self.count(), v)
    }

    #[inline]
    pub fn del(&self, v: T) -> Self {
        AddDel::Del(self.count(), v)
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
