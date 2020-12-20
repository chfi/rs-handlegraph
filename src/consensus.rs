use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use fnv::{FnvHashMap, FnvHashSet};

use rayon::prelude::*;

#[derive(Debug, Clone)]
pub struct LinkPath {
    pub from_cons_name: Vec<u8>,
    pub to_cons_name: Vec<u8>,
    pub from_cons_path: PathId,
    pub to_cons_path: PathId,
    length: usize,
    hash: u64,
    begin: StepPtr,
    end: StepPtr,
    path: PathId,
    is_reverse: bool,
    jump_len: usize,
    rank: u64,
}

impl PartialEq for LinkPath {
    fn eq(&self, other: &Self) -> bool {
        let self_from = &self.from_cons_path;
        let self_to = &self.to_cons_path;
        let other_from = &other.from_cons_path;
        let other_to = &other.to_cons_path;

        (self_from == other_from)
            && (self_to == other_to)
            && (self.length == other.length)
            && (self.hash == other.hash)
    }
}

impl PartialOrd for LinkPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering;

        let self_from = &self.from_cons_path;
        let self_to = &self.to_cons_path;
        let other_from = &other.from_cons_path;
        let other_to = &other.to_cons_path;

        if self_from < other_from {
            return Some(Ordering::Less);
        }

        if self_from == other_from {
            if self_to < other_to {
                return Some(Ordering::Less);
            }

            if self_to == other_to {
                if self.length < other.length {
                    return Some(Ordering::Less);
                }

                if self.length == other.length && self.hash < other.hash {
                    return Some(Ordering::Less);
                }
            }
        }

        if self == other {
            return Some(Ordering::Equal);
        } else {
            return Some(Ordering::Greater);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinkRange {
    start: NodeId,
    end: NodeId,
    path: PathId,
}

pub fn create_consensus_graph(
    smoothed: &PackedGraph,
    consensus_path_names: &[Vec<u8>],
    consensus_jump_max: usize,
    base: Vec<u8>,
) -> PackedGraph {
    let mut res_graph = PackedGraph::default();

    let consensus_paths: Vec<PathId> = consensus_path_names
        .iter()
        .filter_map(|path_name| smoothed.get_path_id(path_name))
        .collect();

    let consensus_path_ptrs: FnvHashMap<PathId, &[u8]> = consensus_path_names
        .iter()
        .filter_map(|path_name| {
            let path_id = smoothed.get_path_id(path_name)?;
            Some((path_id, path_name.as_slice()))
        })
        .collect();

    let is_consensus: Vec<bool> = smoothed
        .path_ids()
        .map(|path_id| consensus_paths.contains(&path_id))
        .collect();

    let mut handle_is_consensus: Vec<bool> = vec![false; smoothed.node_count()];
    let mut handle_consensus_path_ids: Vec<PathId> =
        vec![PathId(0); smoothed.node_count()];

    for &path_id in consensus_paths.iter() {
        if let Some(path_ref) = smoothed.get_path_ref(path_id) {
            for step in path_ref.steps() {
                let node_id = step.handle().id();
                let index = usize::from(node_id) - 1;
                handle_is_consensus[index] = true;
                handle_consensus_path_ids[index] = path_id;
            }
        }
    }

    let non_consensus_paths: Vec<PathId> = smoothed
        .path_ids()
        .filter(|path_id| is_consensus[path_id.0 as usize])
        .collect();

    res_graph
}
