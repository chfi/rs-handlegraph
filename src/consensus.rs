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
    // pub from_cons_name: Vec<u8>,
    // pub to_cons_name: Vec<u8>,
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

impl LinkPath {
    fn new(
        from_cons_path: PathId,
        to_cons_path: PathId,
        path: PathId,
        step: StepPtr,
    ) -> Self {
        Self {
            // from_cons_name: Vec::new(),
            // to_cons_name: Vec::new(),
            from_cons_path,
            to_cons_path,
            length: 0,
            hash: 0,
            path,
            begin: step,
            end: step,
            is_reverse: false,
            jump_len: 0,
            rank: 0,
        }
    }
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

    // let mut link_set: FnvHashSet<LinkPath> = FnvHashSet::default();

    for &path_id in non_consensus_paths.iter() {
        let mut link: Option<LinkPath> = None;

        let path = smoothed.get_path_ref(path_id).unwrap();

        let mut last_seen_consensus: Option<PathId> = None;

        for step in path.steps() {
            // check if we're on the step with any consensus

            let handle = step.handle();
            let node_id = handle.id();
            // let mut on_consensus = false;
            let mut curr_consensus: PathId;

            let curr_consensus =
                if handle_is_consensus[(node_id.0 as usize) - 1] {
                    // on_consensus = true;
                    Some(consensus_paths[(node_id.0 as usize) - 1])
                } else {
                    None
                };

            // if we're on the consensus
            if let Some(curr_cons) = curr_consensus {
                // we haven't seen any consensus before?

                if last_seen_consensus.is_none() {
                    link = Some(LinkPath::new(
                        curr_cons, curr_cons, path_id, step.0,
                    ));
                    last_seen_consensus = Some(curr_cons);
                } else if let Some(link) = link.as_mut() {
                    // let link_ = link.clone().unwrap();

                    let last_handle = smoothed
                        .path_handle_at_step(link.path, link.end)
                        .unwrap();
                    let curr_handle = step.handle();

                    // TODO start_in_vector, end_in_vector
                    let jump_len = 0usize;

                    if Some(link.from_cons_path) == curr_consensus
                        && jump_len < consensus_jump_max
                    {
                        link.begin = step.0;
                        link.end = step.0;
                        link.length = 0;
                        link.is_reverse = step.handle().is_reverse();
                    } else {
                        // or it's different
                        link.to_cons_path = curr_consensus.unwrap();

                        link.end = step.0;

                        link.length = get_path_seq_len(
                            link.path,
                            smoothed
                                .path_next_step(link.path, link.begin)
                                .unwrap(),
                            link.end,
                        );

                        // TODO Seq & hash
                        if link.from_cons_path > link.to_cons_path {
                            std::mem::swap(
                                &mut link.from_cons_path,
                                &mut link.to_cons_path,
                            );
                        }

                        link.jump_len = jump_len;

                        // TODO append link

                        // reset link
                        *link = LinkPath {
                            from_cons_path: curr_consensus.unwrap(),
                            to_cons_path: curr_consensus.unwrap(),
                            length: 0,
                            hash: 0,
                            path: path_id,
                            begin: step.0,
                            end: step.0,
                            is_reverse: step.handle().is_reverse(),
                            jump_len,
                            rank: 0,
                        };
                    }
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

    let most_frequent_link: &LinkPath = unique_links
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

    let mut link_rank = 0u64;

    if has_perfect_edge {
        // nothing, apparently
    } else if let Some(p_link) = perfect_link {
        let mut p_link = p_link.to_owned();
        mark_seen_nodes(
            graph,
            &mut seen_nodes,
            p_link.path,
            p_link.begin,
            p_link.end,
        );
        p_link.rank = link_rank;
        link_rank += 1;
        consensus_links.push(p_link);
    } else if most_frequent_link.from_cons_path
        != most_frequent_link.to_cons_path
    {
        let mut link = most_frequent_link.to_owned();
        link.rank = link_rank;
        link_rank += 1;
        mark_seen_nodes(
            graph,
            &mut seen_nodes,
            link.path,
            link.begin,
            link.end,
        );
        consensus_links.push(link);
    }

    for link in unique_links {
        if link.hash == best_hash {
            continue;
        }
        let novel_bp: usize = novel_seq_len(
            graph,
            &mut seen_nodes,
            link.path,
            link.begin,
            link.end,
        );

        if link.jump_len >= consensus_jump_max || novel_bp >= consensus_jump_max
        {
            let mut link = link.to_owned();

            link.rank = link_rank;
            link_rank += 1;

            mark_seen_nodes(
                graph,
                &mut seen_nodes,
                link.path,
                link.begin,
                link.end,
            );

            consensus_links.push(link);
        }
    }
}
