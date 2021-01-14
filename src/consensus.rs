use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use crate::packedgraph::defragment::*;

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
    // base: Vec<u8>,
) -> PackedGraph {
    println!("using {} consensus paths", consensus_path_names.len());
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

    println!("preparing handle_is_consensus etc.");
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

    println!("building link multiset");
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

    println!("iterating link_multiset");
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

    println!("compute_best_link");
    compute_best_link(
        smoothed,
        consensus_jump_max,
        &curr_links,
        &mut consensus_links,
        &mut perfect_edges,
    );
    println!("compute_best_link - done");

    let mut consensus_graph = PackedGraph::default();

    // consensus path -> smoothed cons path
    let mut path_map: FnvHashMap<PathId, PathId> = FnvHashMap::default();

    println!("adding consensus paths");
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

    println!("adding link paths not in consensus paths");
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

    println!("adding edges");
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

    println!("validating");
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

    crate::algorithms::unchop::unchop(&mut consensus_graph);

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

    let mut updated_links: Vec<(String, Vec<Handle>)> = Vec::new();
    let mut links_to_remove: Vec<PathId> = Vec::new();

    novelify(
        &consensus_graph,
        &consensus_paths_set,
        &mut updated_links,
        &mut links_to_remove,
        consensus_jump_max,
        &links_by_start_end,
    );

    for link in links_to_remove {
        consensus_graph.destroy_path(link);
    }

    let mut link_path_names_to_keep: Vec<String> = Vec::new();

    for (name, handles) in updated_links {
        let path = consensus_graph.create_path(name.as_bytes(), false).unwrap();
        link_path_names_to_keep.push(name);

        for handle in handles {
            consensus_graph.path_append_step(path, handle);
        }
    }

    // remove coverage = 0 nodes

    let empty_handles = consensus_graph
        .handles()
        .filter(|&handle| {
            consensus_graph.steps_on_handle(handle).unwrap().count() != 0
        })
        .collect::<Vec<_>>();

    for handle in empty_handles {
        consensus_graph.remove_handle(handle);
    }

    crate::algorithms::unchop::unchop(&mut consensus_graph);

    // remove edges connecting the same path w/ a gap less than consensus_jump_max
    let mut edges_to_remove: FnvHashSet<Edge> = FnvHashSet::default();
    let mut edges_to_keep: FnvHashSet<Edge> = FnvHashSet::default();

    for path_id in consensus_graph.path_ids() {
        let mut step_to_pos: FnvHashMap<(PathId, StepPtr), usize> =
            FnvHashMap::default();

        let path_ref = consensus_graph.get_path_ref(path_id).unwrap();

        let mut pos = 0usize;
        for step in path_ref.steps() {
            step_to_pos.insert((path_id, step.0), pos);
            pos += consensus_graph.node_len(step.handle());
        }

        for step in path_ref.steps() {
            let key = (path_id, step.0);
            let pos = *step_to_pos.get(&key).unwrap()
                + consensus_graph.node_len(step.handle());

            if consensus_graph.degree(step.handle(), Direction::Right) > 1 {
                for other in
                    consensus_graph.neighbors(step.handle(), Direction::Right)
                {
                    let count =
                        consensus_graph.steps_on_handle(other).unwrap().count();

                    let edge = Edge(step.handle(), other);

                    if edges_to_keep.contains(&edge) {
                        // ok?
                    } else if count > 1 {
                        edges_to_keep.insert(edge);
                    } else {
                        let mut ok = false;

                        for occur in
                            consensus_graph.steps_on_handle(other).unwrap()
                        {
                            // in_path
                            if occur.0 == path_id {
                                let key: (PathId, StepPtr) = (occur.0, occur.1);

                                let o_pos = *step_to_pos.get(&key).unwrap();
                                if o_pos == pos
                                    || o_pos - pos >= consensus_jump_max
                                {
                                    ok = true;
                                }
                            } else {
                                ok = true;
                            }
                        }

                        if !ok && !edges_to_keep.contains(&edge) {
                            edges_to_remove.insert(edge);
                        } else {
                            edges_to_remove.remove(&edge);
                            edges_to_keep.insert(edge);
                        }
                    }
                }
            }
        }
    }

    for edge in edges_to_remove {
        consensus_graph.remove_edge(edge);
    }

    crate::algorithms::unchop::unchop(&mut consensus_graph);

    let mut link_paths = link_paths;
    link_paths.clear();

    for name in link_path_names_to_keep.iter() {
        if let Some(path_id) = consensus_graph.get_path_id(name.as_bytes()) {
            link_paths.push(path_id);
        }
    }

    let mut node_coverage: FnvHashMap<NodeId, usize> = consensus_graph
        .handles()
        .map(|handle| {
            let cov = consensus_graph.steps_on_handle(handle).unwrap().count();
            (handle.id(), cov)
        })
        .collect();

    let mut to_create: Vec<(Vec<u8>, Vec<Handle>)> = Vec::new();

    for &link in link_paths.iter() {
        let mut step = consensus_graph.path_first_step(link).unwrap();

        let mut id = consensus_graph
            .path_handle_at_step(link, step)
            .unwrap()
            .id();

        loop {
            *node_coverage.get_mut(&id).unwrap() -= 1;
            step = consensus_graph.path_next_step(link, step).unwrap();
            id = consensus_graph
                .path_handle_at_step(link, step)
                .unwrap()
                .id();

            if step == consensus_graph.path_last_step(link).unwrap()
                || *node_coverage.get(&id).unwrap() <= 1
            {
                break;
            }
        }

        let begin = step;

        let mut step = consensus_graph.path_last_step(link).unwrap();
        let mut id = consensus_graph
            .path_handle_at_step(link, step)
            .unwrap()
            .id();

        loop {
            *node_coverage.get_mut(&id).unwrap() -= 1;
            step = consensus_graph.path_prev_step(link, step).unwrap();
            id = consensus_graph
                .path_handle_at_step(link, step)
                .unwrap()
                .id();

            if step == consensus_graph.path_first_step(link).unwrap()
                || *node_coverage.get(&id).unwrap() <= 1
            {
                break;
            }
        }

        let end = consensus_graph.path_next_step(link, step).unwrap();

        let mut step = begin;

        let mut new_path: Vec<Handle> = Vec::new();

        loop {
            let handle =
                consensus_graph.path_handle_at_step(link, step).unwrap();
            new_path.push(handle);

            step = consensus_graph.path_next_step(link, step).unwrap();

            if step == end {
                break;
            }
        }

        id = new_path.first().unwrap().id();

        if new_path.is_empty()
            || new_path.len() == 1 && *node_coverage.get(&id).unwrap() > 1
        {
            *node_coverage.get_mut(&id).unwrap() -= 1;
        } else {
            let name = consensus_graph.get_path_name_vec(link).unwrap();
            to_create.push((name, new_path));
        }
    }

    let mut link_path_names_to_keep: Vec<Vec<u8>> = Vec::new();

    for (path_name, handles) in to_create {
        let path = consensus_graph.create_path(&path_name, false).unwrap();
        link_path_names_to_keep.push(path_name);

        for handle in handles {
            consensus_graph.path_append_step(path, handle);
        }
    }

    for &link in link_paths.iter() {
        consensus_graph.destroy_path(link);
    }

    crate::algorithms::unchop::unchop(&mut consensus_graph);

    link_paths.clear();

    link_paths.extend(
        link_path_names_to_keep
            .iter()
            .filter_map(|name| consensus_graph.get_path_id(name)),
    );

    {
        let is_degree_1_tip = |handle: Handle| -> bool {
            let deg_fwd = consensus_graph.degree(handle, Direction::Right);
            let deg_rev = consensus_graph.degree(handle, Direction::Left);

            (deg_fwd == 0 || deg_rev == 0) && (deg_fwd + deg_rev == 1)
        };

        let mut link_tips: Vec<Handle> = Vec::new();
        let mut paths_to_remove: Vec<PathId> = Vec::new();

        for &path in link_paths.iter() {
            let h_first = consensus_graph
                .path_first_step(path)
                .and_then(|step| {
                    consensus_graph.path_handle_at_step(path, step)
                })
                .unwrap();

            if is_degree_1_tip(h_first)
                && consensus_graph.node_len(h_first) < consensus_jump_max
            {
                link_tips.push(h_first);
            }

            let h_last = consensus_graph
                .path_last_step(path)
                .and_then(|step| {
                    consensus_graph.path_handle_at_step(path, step)
                })
                .unwrap();

            if is_degree_1_tip(h_last)
                && consensus_graph.node_len(h_first) < consensus_jump_max
            {
                link_tips.push(h_last);
            }
        }

        for tip in link_tips {
            let to_destroy: Vec<(PathId, StepPtr)> =
                consensus_graph.steps_on_handle(tip).unwrap().collect();

            for (path, step) in to_destroy {
                consensus_graph.path_rewrite_segment(path, step, step, &[]);
            }
        }
    }

    // TODO optimize
    consensus_graph.defragment();

    let empty_handles = consensus_graph
        .handles()
        .filter(|&handle| {
            consensus_graph.steps_on_handle(handle).unwrap().count() != 0
        })
        .collect::<Vec<_>>();

    for handle in empty_handles {
        consensus_graph.remove_handle(handle);
    }

    crate::algorithms::unchop::unchop(&mut consensus_graph);

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

    println!("marking seen nodes");
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
    println!(
        "from_cons_path: {:?} - steps: {:?}",
        from_cons_path,
        graph.path_len(from_cons_path)
    );
    println!(
        "to_cons_path:   {:?} - steps: {:?}",
        to_cons_path,
        graph.path_len(to_cons_path)
    );

    let from_first = graph.path_first_step(from_cons_path).unwrap();
    let from_last = graph.path_last_step(from_cons_path).unwrap();
    let to_first = graph.path_first_step(to_cons_path).unwrap();
    let to_last = graph.path_last_step(to_cons_path).unwrap();

    println!("from_first: {:?}", from_first);
    println!("from_last: {:?}", from_last);
    println!("to_first: {:?}", to_first);
    println!("to_last: {:?}", to_last);

    let from_steps = graph
        .get_path_ref(from_cons_path)
        .unwrap()
        .steps()
        .collect::<Vec<_>>();

    let to_steps = graph
        .get_path_ref(to_cons_path)
        .unwrap()
        .steps()
        .collect::<Vec<_>>();

    println!("from_steps len: {}", from_steps.len());
    println!("from_steps first: {:?}", from_steps.first());
    println!("from_steps last:  {:?}", from_steps.last());

    println!("to_steps len: {}", to_steps.len());
    println!("to_steps first: {:?}", to_steps.first());
    println!("to_steps last:  {:?}", to_steps.last());

    let to_end_fwd = graph.path_handle_at_step(to_cons_path, to_last).unwrap();
    let to_end_rev = to_end_fwd.flip();

    let from_begin_fwd = graph
        .path_handle_at_step(from_cons_path, from_first)
        .unwrap();
    let from_begin_rev = from_begin_fwd.flip();

    let to_begin_fwd: Handle =
        graph.path_handle_at_step(to_cons_path, to_first).unwrap();
    let to_begin_rev = to_begin_fwd.flip();

    let from_end_fwd: Handle = graph
        .path_handle_at_step(from_cons_path, from_last)
        .unwrap();
    let from_end_rev = from_end_fwd.flip();

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
        println!("iterating unique_links - {}", unique_links.len());
        for link in unique_links.iter() {
            let mut step = link.begin;

            let mut count = 0;
            loop {
                println!("   {}", count);
                count += 1;

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

                step = next;

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

fn novelify(
    consensus: &PackedGraph,
    consensus_paths_set: &FnvHashSet<PathId>,
    updated_links: &mut Vec<(String, Vec<Handle>)>,
    links_to_remove: &mut Vec<PathId>,
    consensus_jump_max: usize,
    group: &[LinkRange],
) {
    let mut internal_nodes: FnvHashSet<NodeId> = FnvHashSet::default();

    for &link_range in group {
        let path_ref = consensus.get_path_ref(link_range.path).unwrap();
        internal_nodes.extend(path_ref.steps().map(|step| step.handle().id()));
    }

    let mut seen_nodes: FnvHashSet<NodeId> = FnvHashSet::default();
    let mut reached_ext_nodes: FnvHashSet<NodeId> = FnvHashSet::default();

    for &link in group {
        links_to_remove.push(link.path);

        let begin = consensus.path_first_step(link.path).unwrap();
        let end = consensus.path_last_step(link.path).unwrap();

        let mut in_novel = false;
        let mut reaches_external = false;

        let mut novel_bp = 0usize;
        let mut first_novel = None;

        let mut save_rank = 0usize;

        let path_ref = consensus.get_path_ref(link.path).unwrap();

        for step in path_ref.steps() {
            if !seen_nodes.contains(&step.handle().id()) {
                let mut mark_ext = |other: Handle| {
                    let o_id = other.id();

                    if !reached_ext_nodes.contains(&o_id) {
                        reached_ext_nodes.insert(o_id);
                        for (path_id, _step_ptr) in
                            consensus.steps_on_handle(other).unwrap()
                        {
                            let path_consensus =
                                consensus_paths_set.contains(&path_id);
                            reaches_external |= path_consensus;
                        }
                    }
                };

                consensus
                    .neighbors(step.handle(), Direction::Right)
                    .for_each(&mut mark_ext);

                consensus
                    .neighbors(step.handle(), Direction::Left)
                    .for_each(mark_ext);

                seen_nodes.insert(step.handle().id());

                novel_bp += consensus.node_len(step.handle());
                if !in_novel {
                    first_novel = Some(step.0);
                    in_novel = true;
                }
            } else {
                if in_novel {
                    in_novel = false;
                    if reaches_external || novel_bp >= consensus_jump_max {
                        save_path_fragment(
                            consensus,
                            updated_links,
                            link.path,
                            &mut save_rank,
                            first_novel.unwrap(),
                            step.0,
                        );
                        reaches_external = false;
                        novel_bp = 0;
                    }
                }
            }
        }
        if in_novel {
            if reaches_external || novel_bp >= consensus_jump_max {
                save_path_fragment(
                    consensus,
                    updated_links,
                    link.path,
                    &mut save_rank,
                    first_novel.unwrap(),
                    end,
                );
            }
        }
    }
}
