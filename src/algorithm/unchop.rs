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

    // remove the old nodes
    for &handle in handles.iter() {
        graph.remove_handle(handle);
    }

    Some(new_handle)
}

fn combine_handles(
    graph: &mut PackedGraph,
    handles: &[Handle],
) -> Option<Handle> {
    //
    unimplemented!();
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
