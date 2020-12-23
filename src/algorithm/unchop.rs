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

    for &other in neighbors.iter() {
        graph.create_edge(Edge(other, left));
    }

    // create the right neighbors
    neighbors.clear();
    neighbors.extend(graph.neighbors(right, Direction::Right));

    for &other in neighbors.iter() {
        graph.create_edge(Edge(right, other));
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
}
