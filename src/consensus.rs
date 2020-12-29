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

    let mut handle_consensus_path_ids: FnvHashMap<NodeId, Vec<PathId>> =
        FnvHashMap::default();

    // let mut handle_consensus_path_ids: Vec<PathId> =
    //     vec![PathId(0); smoothed.node_count()];

    for &path_id in consensus_paths.iter() {
        if let Some(path_ref) = smoothed.get_path_ref(path_id) {
            for step in path_ref.steps() {
                let node_id = step.handle().id();
                let index = usize::from(node_id) - 1;
                handle_is_consensus[index] = true;

                handle_consensus_path_ids
                    .entry(node_id)
                    .or_default()
                    .push(path_id);
                // handle_consensus_path_ids[index] = path_id;
            }
        }
    }

    let non_consensus_paths: Vec<PathId> = smoothed
        .path_ids()
        .filter(|path_id| is_consensus[path_id.0 as usize])
        .collect();

    let get_path_seq_len =
        |path: PathId, begin: StepPtr, end: StepPtr| -> usize {
            let mut step = begin;
            let mut len = 0;
            while step != end {
                let handle = smoothed.path_handle_at_step(path, step).unwrap();
                len += smoothed.node_len(handle);
                step = smoothed.path_next_step(path, step).unwrap();
            }

            len
        };

    let get_path_seq =
        |path: PathId, begin: StepPtr, end: StepPtr| -> Vec<u8> {
            let mut step = begin;
            let mut seq = Vec::new();
            while step != end {
                let handle = smoothed.path_handle_at_step(path, step).unwrap();
                seq.extend(smoothed.sequence(handle));
                step = smoothed.path_next_step(path, step).unwrap();
            }

            seq
        };

    let add_path_segment = |link: &LinkPath,
                            begin: StepPtr,
                            end: StepPtr,
                            cns: &mut PackedGraph| {
        unimplemented!();
    };

    for &nc_path in non_consensus_paths.iter() {
        let mut link: LinkPath;

        let path = smoothed.get_path_ref(nc_path).unwrap();

        let mut last_seen_consensus: Option<PathId> = None;
        let mut on_consensus = false;

        for step in path.steps() {
            // check if we're on the step with any consensus

            // if we're on the consensus
            if on_consensus {
                // we haven't seen any consensus before?
                if last_seen_consensus.is_none() {
                } else {
                    /*
                        if link.from_cons_path == curr_consensus
                        && jump_length < consensus_jump_max {
                        link.begin = step;
                        link.end = step;
                        link.length = 0;
                    } else { // or it's different
                    }
                        */
                }
            } else {
            }
        }
    }

    res_graph
}

fn novel_seq_len(
    graph: &PackedGraph,
    seen_nodes: &mut FnvHashSet<NodeId>,
    path: PathId,
    begin: StepPtr,
    end: StepPtr,
) -> usize {
    let mut novel_bp = 0usize;

    let mut step = begin;

    loop {
        let handle = graph.path_handle_at_step(path, step).unwrap();
        let id = handle.id();

        if !seen_nodes.contains(&id) {
            novel_bp += graph.node_len(handle);
            seen_nodes.insert(id);
        }

        if step == end {
            break;
        }

        step = graph.path_next_step(path, step).unwrap();
    }

    novel_bp
}

fn mark_seen_nodes(
    graph: &PackedGraph,
    seen_nodes: &mut FnvHashSet<NodeId>,
    path: PathId,
    begin: StepPtr,
    end: StepPtr,
) {
    let mut step = begin;

    loop {
        let handle = graph.path_handle_at_step(path, step).unwrap();
        let id = handle.id();

        if !seen_nodes.contains(&id) {
            seen_nodes.insert(id);
        }

        if step == end {
            break;
        }

        step = graph.path_next_step(path, step).unwrap();
    }
}

fn compute_best_link(
    graph: &PackedGraph,
    consensus_jump_max: usize,
    links: &[LinkPath],
    consensus_links: &mut Vec<LinkPath>,
    perfect_edges: &mut Vec<(Handle, Handle)>,
) {
    let mut hash_counts: FnvHashMap<u64, u64> = FnvHashMap::default();
    let mut unique_links: Vec<&LinkPath> = Vec::new();

    for link in links {
        let c = hash_counts.entry(link.hash).or_default();
        if *c == 0 {
            unique_links.push(link);
        }
        *c += 1;
    }

    let hash_lengths: FnvHashMap<u64, usize> =
        links.iter().map(|link| (link.hash, link.length)).collect();

    let (&best_hash, &best_count) =
        hash_counts.iter().max_by_key(|(_, c)| *c).unwrap();

    let most_frequent_link = unique_links
        .iter()
        .find(|&&link| link.hash == best_hash)
        .copied()
        .unwrap();

    let from_cons_path = most_frequent_link.from_cons_path;
    let to_cons_path = most_frequent_link.to_cons_path;

    let from_first = graph.path_first_step(from_cons_path).unwrap();
    let from_last = graph.path_last_step(from_cons_path).unwrap();
    let to_first = graph.path_first_step(to_cons_path).unwrap();
    let to_last = graph.path_last_step(to_cons_path).unwrap();

    let from_end_fwd: Handle = graph
        .path_handle_at_step(from_cons_path, from_last)
        .unwrap();
    let from_end_rev = from_end_fwd.flip();

    let to_begin_fwd: Handle =
        graph.path_handle_at_step(to_cons_path, to_first).unwrap();
    let to_begin_rev = to_begin_fwd.flip();

    let from_begin_fwd = graph
        .path_handle_at_step(from_cons_path, from_first)
        .unwrap();
    let from_begin_rev = from_begin_fwd.flip();

    let to_end_fwd = graph.path_handle_at_step(to_cons_path, to_last).unwrap();
    let to_end_rev = to_end_fwd.flip();

    let mut has_perfect_edge = false;
    let mut has_perfect_link = false;
    let mut perfect_link: Option<LinkPath> = None;

    if graph.has_edge(from_end_fwd, to_begin_fwd) {
        perfect_edges.push((from_end_fwd, to_begin_fwd));
        has_perfect_edge = true;
    } else if graph.has_edge(to_end_fwd, from_begin_fwd) {
        perfect_edges.push((to_end_fwd, from_begin_fwd));
        has_perfect_edge = true;
    } else {
        for link in unique_links.iter() {
            let mut step = link.begin;

            loop {
                let next = graph.path_next_step(link.path, step).unwrap();

                let b: Handle =
                    graph.path_handle_at_step(link.path, step).unwrap();
                let e: Handle =
                    graph.path_handle_at_step(link.path, next).unwrap();

                if b == from_end_fwd && e == to_begin_fwd
                    || b == from_end_rev && e == to_begin_rev
                    || b == to_begin_fwd && e == from_end_fwd
                    || b == to_begin_rev && e == from_end_rev
                {
                    has_perfect_link = true;
                    perfect_link = Some(link.clone().clone());
                    break;
                }

                if step == link.end {
                    break;
                }
            }

            if has_perfect_link {
                break;
            }
        }
    }

    // let perfect_link = perfect_link.unwrap();

    let mut seen_nodes: FnvHashSet<NodeId> = FnvHashSet::default();

    // TODO the original implementation requires multiple mutable
    // references to elements in unique_links, it looks like; need
    // to fix that

    if has_perfect_edge {
        // nothing, apparently
    } else if let Some(p_link) = perfect_link {
        // TODO mark_seen_nodes()
        // p_link.rank += 1;
        consensus_links.push(p_link.clone());
    } else if most_frequent_link.from_cons_path
        != most_frequent_link.to_cons_path
    {

        // most_frequent_link.lank +=
    }

    let mut link_rank = 0u64;

    for link in unique_links {
        if link.hash == best_hash {
            continue;
        }
        // TODO novel_sequence_length
        let novel_bp: usize = novel_seq_len(
            graph,
            &mut seen_nodes,
            link.path,
            link.begin,
            link.end,
        );

        if link.jump_len >= consensus_jump_max || novel_bp >= consensus_jump_max
        {
            // TODO link.rank = link_rank;
            link_rank += 1;
            consensus_links.push(link.clone());
            // TODO mark_seen_nodes
        }
    }
}
