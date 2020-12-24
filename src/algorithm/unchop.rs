use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

use fnv::{FnvHashMap, FnvHashSet};

use rayon::prelude::*;

/// Merge the handles in the provided slice to a single node, whose
/// sequence is the concatenation of the handles' sequences
/// left-to-right.
///
/// The handles must be in left-to-right order and share orientation,
/// and all paths that contain any of the handles must contain all
/// other handles, in the same order as in the slice (in either
/// direction).
fn concat_nodes(graph: &mut PackedGraph, handles: &[Handle]) -> Option<Handle> {
    if handles.len() < 2 {
        return None;
    }

    // TODO check paths, order, orientation

    let left = *handles.first()?;
    let right = *handles.last()?;

    // create the new node
    let new_seq: Vec<u8> = handles
        .iter()
        .flat_map(|handle| graph.sequence(*handle))
        .collect();

    let new_handle = graph.append_handle(&new_seq);

    // create the left neighbors
    let mut neighbors =
        graph.neighbors(left, Direction::Left).collect::<Vec<_>>();

    let mut left_neighbors: FnvHashSet<Handle> = FnvHashSet::default();

    for &other in neighbors.iter() {
        if Some(other) == handles.last().copied() {
            left_neighbors.insert(new_handle);
        } else if Some(other) == handles.first().copied().map(Handle::flip) {
            left_neighbors.insert(new_handle.flip());
        } else {
            left_neighbors.insert(other);
        }
    }

    // create the right neighbors
    neighbors.clear();
    neighbors.extend(graph.neighbors(right, Direction::Right));

    let mut right_neighbors: FnvHashSet<Handle> = FnvHashSet::default();

    for other in neighbors {
        if Some(other) == handles.first().copied() {
            // right_neighbors.insert(new_handle);
        } else if Some(other) == handles.last().copied().map(Handle::flip) {
            right_neighbors.insert(new_handle.flip());
        } else {
            right_neighbors.insert(other);
        }
    }

    for other in left_neighbors {
        graph.create_edge(Edge(other, new_handle));
    }

    for other in right_neighbors {
        graph.create_edge(Edge(new_handle, other));
    }

    // TODO update paths
    let mut to_rewrite: Vec<(PathId, StepPtr, StepPtr, bool)> = Vec::new();

    for (path_id, front_step) in
        graph.steps_on_handle(*handles.first().unwrap()).unwrap()
    {
        let runs_reverse = graph.path_handle_at_step(path_id, front_step)
            != handles.first().copied();

        let mut back_step = front_step;

        let last_step = if runs_reverse {
            let h = *handles.last().unwrap();
            h.flip()
        } else {
            *handles.last().unwrap()
        };

        loop {
            if graph.path_handle_at_step(path_id, back_step).unwrap()
                == last_step
            {
                break;
            }

            back_step = if runs_reverse {
                graph.path_prev_step(path_id, back_step).unwrap()
            } else {
                graph.path_next_step(path_id, back_step).unwrap()
            };
        }

        if runs_reverse {
            to_rewrite.push((path_id, back_step, front_step, true));
        } else {
            to_rewrite.push((path_id, front_step, back_step, false));
        }
    }

    for (path_id, from, to, rev) in to_rewrite {
        let new_seg = if rev { new_handle.flip() } else { new_handle };
        graph.path_rewrite_segment(path_id, from, to, &[new_seg]);
    }

    // remove the old nodes
    for &handle in handles.iter() {
        graph.remove_handle(handle);
    }

    Some(new_handle)
}

pub fn unchop(graph: &mut PackedGraph) {
    //
    // let mut node_rank: FnvHashMap<NodeId, usize> = FnvHashMap::default();
    // node_rank.reserve(graph.node_count());

    let mut node_rank: FnvHashMap<NodeId, usize> = graph
        .handles()
        .enumerate()
        .map(|(rank, handle)| (handle.id(), rank))
        .collect();

    // TODO find the components to merge
}
