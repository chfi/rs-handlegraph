use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use fnv::{FnvHashMap, FnvHashSet};

use boomphf::*;

use red_union_find::UF;

use rayon::prelude::*;

pub mod unchop;

pub fn simple_components(
    graph: &PackedGraph,
    min_size: usize,
) -> Vec<Vec<Handle>> {
    let mut bphf_data = Vec::with_capacity(graph.node_count() * 2 + 1);
    for handle in graph.handles() {
        bphf_data.push(handle.0);
        bphf_data.push(handle.flip().0);
    }

    let bphf = Mphf::new_parallel(1.7, &bphf_data, None);

    // TODO this isn't strictly portable, but UF<T> requires T:
    // Into<usize>, which u64 doesn't implement, at least not on
    // stable -- this should work fine, though.
    let mut union_find: UF<usize> = UF::new_reflexive(bphf_data.len() + 1);
    for handle in graph.handles() {
        let h_i = bphf.hash(&handle.0);
        let h_j = bphf.hash(&handle.flip().0);
        union_find.union(h_i as usize, h_j as usize);
    }

    for Edge(from, to) in graph.edges() {
        if from.id() == to.id()
            && graph.degree(from, Direction::Right) == 1
            && graph.degree(to, Direction::Left) == 1
            && perfect_neighbors(graph, from, to)
        {
            let from = bphf.hash(&from.0);
            let to = bphf.hash(&to.0);
            union_find.union(from as usize, to as usize);
        }
    }

    let mut simple_components: FnvHashMap<u64, Vec<Handle>> =
        FnvHashMap::default();

    for handle in graph.handles() {
        let a_id = union_find.find(handle.0 as usize);
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
                graph
                    .neighbors(handle, Direction::Left)
                    .for_each(|p| prev = p);
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
                graph
                    .neighbors(handle, Direction::Right)
                    .for_each(|p| next = p);
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
        let step_is_rev =
            graph.path_handle_at_step(path_id, step_ptr).unwrap() != left;

        let next_step = if step_is_rev {
            graph.path_prev_step(path_id, step_ptr)
        } else {
            graph.path_next_step(path_id, step_ptr)
        };

        match next_step {
            None => return false,
            Some(next_step) => {
                let mut next_handle =
                    graph.path_handle_at_step(path_id, next_step).unwrap();
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
