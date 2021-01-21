use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::index::OneBasedIndex;
use crate::packedgraph::*;

use fnv::{FnvHashMap, FnvHashSet};

use boomphf::*;

use crate::disjoint::DisjointSets;

use rayon::prelude::*;

#[allow(unused_imports)]
use log::{debug, error, info, trace};

pub mod unchop;

pub fn simple_components(
    graph: &PackedGraph,
    min_size: usize,
) -> Vec<Vec<Handle>> {
    debug!("in simple components");
    let mut bphf_data = Vec::with_capacity((1 + graph.node_count()) * 2);

    for handle in graph.handles() {
        bphf_data.push(handle.0);
        bphf_data.push(handle.flip().0);
    }

    let bphf = Mphf::new_parallel(1.7, &bphf_data, None);

    let disj_set = DisjointSets::new(bphf_data.len() + 1);

    graph.handles_par().for_each(|handle| {
        let h_i = bphf.hash(&handle.0);
        let h_j = bphf.hash(&handle.flip().0);
        disj_set.unite(h_i, h_j);
    });

    graph.edges_par().for_each(|Edge(from, to)| {
        if from.id() != to.id()
            && graph.degree(from, Direction::Right) == 1
            && graph.degree(to, Direction::Left) == 1
            && perfect_neighbors(graph, from, to)
        {
            let from = bphf.hash(&from.0);
            let to = bphf.hash(&to.0);
            disj_set.unite(from, to);
        }
    });

    let mut simple_components: FnvHashMap<u64, Vec<Handle>> =
        FnvHashMap::default();

    for handle in graph.handles() {
        let a_id = disj_set.find(bphf.hash(&handle.0));
        simple_components
            .entry(a_id as u64)
            .or_default()
            .push(handle);
    }

    let mut handle_components: Vec<Vec<Handle>> = Vec::new();

    for comp in simple_components.values_mut() {
        if comp.len() < min_size {
            continue;
        }

        comp.sort_by(|a, b| b.cmp(a));

        let comp_set: FnvHashSet<Handle> = comp.iter().copied().collect();

        let mut handle = *comp.first().unwrap();
        let base = handle;
        let mut has_prev: bool;

        loop {
            has_prev = graph.degree(handle, Direction::Left) == 1;
            let mut prev = handle;

            if has_prev {
                prev = graph.neighbors(handle, Direction::Left).next().unwrap();
            }

            if handle != prev && prev != base && comp_set.contains(&prev) {
                handle = prev;
            } else {
                break;
            }
        }

        let base = handle;
        let mut has_next: bool;

        let mut sorted_comp: Vec<Handle> = Vec::new();

        loop {
            sorted_comp.push(handle);
            has_next = graph.degree(handle, Direction::Right) == 1;
            let mut next = handle;

            if has_next {
                next =
                    graph.neighbors(handle, Direction::Right).next().unwrap();
            }

            if handle != next && next != base && comp_set.contains(&next) {
                handle = next;
            } else {
                break;
            }
        }

        if sorted_comp.len() >= min_size {
            handle_components.push(sorted_comp);
        }
    }

    handle_components
}

pub fn perfect_neighbors(
    graph: &PackedGraph,
    left: Handle,
    right: Handle,
) -> bool {
    let mut expected_next = 0usize;

    for (path_id, step_ptr) in graph.steps_on_handle(left).unwrap() {
        let step =
            graph
                .path_handle_at_step(path_id, step_ptr)
                .unwrap_or_else(|| {
                    let first = graph.path_first_step(path_id).unwrap();
                    let last = graph.path_last_step(path_id).unwrap();
                    panic!(
                        "path {}, first {} last {}, missing step {}",
                        path_id.0,
                        first.to_vector_value(),
                        last.to_vector_value(),
                        step_ptr.to_vector_value()
                    );
                });

        let step_is_rev = step != left;

        let next_step = if step_is_rev {
            graph.path_prev_step(path_id, step_ptr)
        } else {
            graph.path_next_step(path_id, step_ptr)
        };

        match next_step {
            None => return false,
            Some(next_step) => {
                let mut next_handle = graph
                    .path_handle_at_step(path_id, next_step)
                    .unwrap_or_else(|| {
                        panic!(
                            "error getting step: path {}, ptr {}",
                            path_id.0,
                            next_step.to_vector_value()
                        );
                    });
                if step_is_rev {
                    next_handle = next_handle.flip();
                }

                if next_handle != right {
                    return false;
                } else {
                    expected_next += 1;
                }
            }
        }
    }

    let observed_next = graph.steps_on_handle(right).unwrap().count();

    observed_next == expected_next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hnd(x: u64) -> Handle {
        Handle::pack(x, false)
    }

    fn vec_hnd(v: Vec<u64>) -> Vec<Handle> {
        v.into_iter().map(hnd).collect::<Vec<_>>()
    }

    #[test]
    fn simple_components_basic() {
        let mut graph = PackedGraph::default();

        let n1 = graph.append_handle(b"CAAATAAG");
        let n2 = graph.append_handle(b"A");
        let n3 = graph.append_handle(b"G");
        let n4 = graph.append_handle(b"T");
        let n5 = graph.append_handle(b"C");
        let n6 = graph.append_handle(b"TTG");

        graph.create_edge(Edge(n1, n2));
        graph.create_edge(Edge(n1, n5));
        graph.create_edge(Edge(n2, n3));
        graph.create_edge(Edge(n5, n6));
        graph.create_edge(Edge(n3, n4));
        graph.create_edge(Edge(n6, n4));

        let comps = simple_components(&graph, 2);

        assert_eq!(comps, vec![vec_hnd(vec![2, 3]), vec_hnd(vec![5, 6])]);
    }
}
