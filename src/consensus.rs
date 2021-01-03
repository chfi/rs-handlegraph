use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use fnv::FnvHasher;
use fnv::{FnvHashMap, FnvHashSet};

use rayon::prelude::*;

use std::hash::{Hash, Hasher};

use bstr::ByteSlice;

#[derive(Debug, Clone, Hash)]
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

impl Eq for LinkPath {}

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

    let is_consensus: FnvHashMap<PathId, bool> = smoothed
        .path_ids()
        .map(|path_id| (path_id, consensus_paths.contains(&path_id)))
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
        .filter(|path_id| is_consensus[path_id])
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

    // we're emulating a multiset by having the key be the hash field
    // of the link path, and then the value a hashset, where
    // LinkPath's Hash impl is the derived one
    let mut link_multiset: FnvHashMap<u64, FnvHashSet<LinkPath>> =
        FnvHashMap::default();

    for &path_id in non_consensus_paths.iter() {
        let mut link: Option<LinkPath> = None;

        let path = smoothed.get_path_ref(path_id).unwrap();

        let mut last_seen_consensus: Option<PathId> = None;

        for step in path.steps() {
            // check if we're on the step with any consensus

            let handle = step.handle();
            let node_id = handle.id();

            let ix = node_id.0 as usize - 1;

            let curr_consensus = if handle_is_consensus[ix] {
                // on_consensus = true;
                handle_consensus_path_ids
                    .get(&node_id)
                    .and_then(|x| x.first())
                    .copied()
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
                    let last_handle = smoothed
                        .path_handle_at_step(link.path, link.end)
                        .unwrap();
                    let curr_handle = step.handle();

                    let jump_len = {
                        let start =
                            start_in_vector(&smoothed, curr_handle) as isize;
                        let end =
                            end_in_vector(&smoothed, last_handle) as isize;
                        let diff = (start - end).abs();
                        diff as usize
                    };

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

                        let mut hasher = FnvHasher::default();

                        let seq = get_path_seq(
                            path_id,
                            smoothed
                                .path_next_step(path_id, link.begin)
                                .unwrap(),
                            link.end,
                        );

                        let beg_h = smoothed
                            .path_handle_at_step(path_id, link.begin)
                            .unwrap();
                        let end_h = smoothed
                            .path_handle_at_step(path_id, link.end)
                            .unwrap();

                        let handle_str = format!(
                            "{}:{}:{}",
                            u64::from(beg_h.id()),
                            u64::from(end_h.id()),
                            seq.as_bstr()
                        );

                        handle_str.hash(&mut hasher);
                        link.hash = hasher.finish();

                        if link.from_cons_path > link.to_cons_path {
                            std::mem::swap(
                                &mut link.from_cons_path,
                                &mut link.to_cons_path,
                            );
                        }

                        link.jump_len = jump_len;

                        let mut new_link = LinkPath {
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

                        // swap to reset the link
                        std::mem::swap(&mut new_link, link);

                        // append the link
                        link_multiset
                            .entry(new_link.hash)
                            .or_default()
                            .insert(new_link);
                    }
                }
            } /* else {
              }*/
        }
    }

    let mut perfect_edges: Vec<(Handle, Handle)> = Vec::new();

    let mut consensus_links: Vec<LinkPath> = Vec::new();
    let mut curr_links: Vec<&LinkPath> = Vec::new();

    let mut curr_from_cons_path: Option<PathId> = None;
    let mut curr_to_cons_path: Option<PathId> = None;

    for link_path_set in link_multiset.values() {
        for link_path in link_path_set.iter() {
            if curr_links.is_empty() {
                curr_from_cons_path = Some(link_path.from_cons_path);
                curr_to_cons_path = Some(link_path.to_cons_path);
            } else if curr_from_cons_path != Some(link_path.from_cons_path)
                || curr_to_cons_path != Some(link_path.to_cons_path)
            {
                compute_best_link(
                    smoothed,
                    consensus_jump_max,
                    &curr_links,
                    &mut consensus_links,
                    &mut perfect_edges,
                );

                curr_links.clear();
                curr_from_cons_path = Some(link_path.from_cons_path);
                curr_to_cons_path = Some(link_path.to_cons_path);
            }
            curr_links.push(link_path);
        }
    }

    compute_best_link(
        smoothed,
        consensus_jump_max,
        &curr_links,
        &mut consensus_links,
        &mut perfect_edges,
    );

    let mut consensus_graph = PackedGraph::default();

    // consensus path -> smoothed cons path
    let mut path_map: FnvHashMap<PathId, PathId> = FnvHashMap::default();

    // add consensus paths to consensus graph
    for &path_id in consensus_paths.iter() {
        let path_name = smoothed.get_path_name_vec(path_id).unwrap();

        let new_path_id =
            consensus_graph.create_path(&path_name, false).unwrap();

        path_map.insert(new_path_id, path_id);

        let path_ref = smoothed.get_path_ref(path_id).unwrap();

        for step in path_ref.steps() {
            let handle = step.handle();

            if !consensus_graph.has_node(handle.id()) {
                let seq = smoothed.sequence_vec(handle);
                consensus_graph.create_handle(&seq, handle.id());
            }
        }
    }

    consensus_graph.with_all_paths_mut_ctx_chn_new(
        |cons_path_id, sender, cons_path_ref| {
            let path_id = *path_map.get(&cons_path_id).unwrap();
            let path_ref = smoothed.get_path_ref(path_id).unwrap();

            cons_path_ref.append_handles_iter_chn(
                sender,
                path_ref.steps().map(|step| step.handle()),
            );
        },
    );

    // add link paths not in the consensus paths
    let mut link_path_names: Vec<String> = Vec::new();

    for link in consensus_links.iter() {
        if link.length > 0 {
            let from_cons_name =
                smoothed.get_path_name_vec(link.from_cons_path).unwrap();
            let to_cons_name =
                smoothed.get_path_name_vec(link.to_cons_path).unwrap();
            let link_name = format!(
                "Link_{}_{}_{}",
                from_cons_name.as_bstr(),
                to_cons_name.as_bstr(),
                link.rank
            );

            let path_cons_graph = consensus_graph
                .create_path(link_name.as_bytes(), false)
                .unwrap();

            link_path_names.push(link_name);

            let mut step = link.begin;

            loop {
                let handle =
                    smoothed.path_handle_at_step(link.path, step).unwrap();

                let mut cons_handle = if !consensus_graph.has_node(handle.id())
                {
                    let seq = smoothed.sequence_vec(handle);
                    consensus_graph.create_handle(&seq, handle.id())
                } else {
                    handle
                };

                if handle.is_reverse() {
                    cons_handle = cons_handle.flip();
                }

                consensus_graph.path_append_step(path_cons_graph, cons_handle);

                if step == link.end {
                    break;
                }

                step = smoothed.path_next_step(link.path, step).unwrap();
            }
        }
    }

    // add the edges
    let mut edges: FnvHashSet<Edge> = FnvHashSet::default();

    for path_id in consensus_graph.path_ids() {
        let path_ref = consensus_graph.get_path_ref(path_id).unwrap();
        let mut steps = path_ref.steps();
        let mut prev = steps.next().unwrap();

        for step in steps {
            let prev_h = prev.handle();
            let curr_h = step.handle();

            edges.insert(Edge(prev_h, curr_h));
            prev = step;
        }
    }

    for (from, to) in perfect_edges {
        edges.insert(Edge(from, to));
    }

    consensus_graph.create_edges_iter(edges.into_iter());

    {
        let mut link_steps = |path_id: PathId,
                              step_a: StepPtr,
                              step_b: StepPtr| {
            let from = smoothed.path_handle_at_step(path_id, step_a).unwrap();
            let to = smoothed.path_handle_at_step(path_id, step_b).unwrap();

            if consensus_graph.has_node(from.id())
                && consensus_graph.has_node(to.id())
            {
                consensus_graph.create_edge(Edge(from, to));
            }
        };

        for link in consensus_links.iter() {
            let next = smoothed.path_next_step(link.path, link.begin).unwrap();

            link_steps(link.path, link.begin, next);

            let prev = smoothed.path_prev_step(link.path, link.end).unwrap();
            if prev != link.begin {
                link_steps(link.path, prev, link.end);
            }
        }
    }

    // validation

    for path_id in smoothed.path_ids().filter(|p| consensus_paths.contains(&p))
    {
        let path_name = smoothed.get_path_name_vec(path_id).unwrap();

        if consensus_graph.get_path_id(&path_name).is_none() {
            panic!(
                "error: consensus path {} not present in consensus graph",
                path_name.as_bstr()
            );
        }

        let path_ref = smoothed.get_path_ref(path_id).unwrap();

        for step in path_ref.steps() {
            let s_seq = smoothed.sequence(step.handle());
            let c_seq = consensus_graph.sequence(step.handle());
            assert!(
                s_seq.eq(c_seq),
                "error: node {} has different sequences in the graphs",
                step.handle().id()
            );
        }
    }

    let consensus_graph_path_ids =
        consensus_graph.path_ids().collect::<Vec<_>>();

    for path_id in consensus_graph_path_ids {
        let path_ref = smoothed.get_path_ref(path_id).unwrap();

        let mut steps = path_ref.steps();
        let first = steps.next().unwrap();

        let edges_iter = steps.scan(first, |prev, curr| {
            let edge = Edge(prev.handle(), curr.handle());
            *prev = curr;
            Some(edge)
        });

        consensus_graph.create_edges_iter(edges_iter);
    }

    crate::algorithm::unchop::unchop(&mut consensus_graph);

    let link_paths = link_path_names
        .iter()
        .filter_map(|name| consensus_graph.get_path_id(name.as_bytes()))
        .collect::<Vec<_>>();

    let mut consensus_paths = consensus_paths;
    consensus_paths.clear();

    let consensus_paths_set: FnvHashSet<PathId> = consensus_path_names
        .iter()
        .filter_map(|name| {
            let path_id = consensus_graph.get_path_id(name.as_bytes())?;

            consensus_paths.push(path_id);
            Some(path_id)
        })
        .collect();

    // remove paths that are contained in others
    // and add less than consensus_jump_max bp of sequence

    let mut links_by_start_end = link_paths
        .iter()
        .filter_map(|&link_path| {
            let first = consensus_graph.path_first_step(link_path)?;
            let last = consensus_graph.path_last_step(link_path)?;

            let h_a = consensus_graph.path_handle_at_step(link_path, first)?;
            let h_b = consensus_graph.path_handle_at_step(link_path, last)?;

            let start = h_a.id().min(h_b.id());
            let end = h_a.id().max(h_b.id());

            Some(LinkRange {
                start,
                end,
                path: link_path,
            })
        })
        .collect::<Vec<_>>();

    links_by_start_end.par_sort_by(|a, b| {
        use std::cmp::Ordering;
        if a.start < b.end {
            Ordering::Less
        } else if a.start == b.end && a.end > b.end {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });

    consensus_graph
}

fn start_in_vector(graph: &PackedGraph, handle: Handle) -> usize {
    let (offset, len) = graph.nodes.get_node_seq_range(handle).unwrap();
    if handle.is_reverse() {
        offset + len
    } else {
        offset
    }
}

fn end_in_vector(graph: &PackedGraph, handle: Handle) -> usize {
    let (offset, len) = graph.nodes.get_node_seq_range(handle).unwrap();
    if handle.is_reverse() {
        offset
    } else {
        offset + len
    }
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
    links: &[&LinkPath],
    consensus_links: &mut Vec<LinkPath>,
    perfect_edges: &mut Vec<(Handle, Handle)>,
) {
    let mut hash_counts: FnvHashMap<u64, u64> = FnvHashMap::default();
    let mut unique_links: Vec<&LinkPath> = Vec::new();

    for link in links {
        let c = hash_counts.entry(link.hash).or_default();
        if *c == 0 {
            unique_links.push(*link);
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

fn save_path_fragment(
    consensus: &PackedGraph,
    updated_links: &mut Vec<(String, Vec<Handle>)>,
    link: PathId,
    save_rank: &mut usize,
    first_novel: StepPtr,
    step: StepPtr,
) {
    let path_name = consensus.get_path_name_vec(link).unwrap();
    let string = format!("{}_{}", path_name.as_bstr(), save_rank);
    *save_rank += 1;

    let mut handles = Vec::new();

    let mut q = first_novel;

    loop {
        let handle = consensus.path_handle_at_step(link, q).unwrap();
        handles.push(handle);

        q = consensus.path_next_step(link, q).unwrap();

        if q == step {
            break;
        }
    }

    updated_links.push((string, handles));
}
