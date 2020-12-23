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

pub mod unchop;

pub fn perfect_neighbors(
    graph: &PackedGraph,
    left: Handle,
    right: Handle,
) -> bool {
    let mut perfect = true;
    let mut expected_next = 0usize;

    for (path_id, step_ptr) in graph.steps_on_handle(left).unwrap() {
        let step_is_rev =
            graph.path_handle_at_step(path_id, step_ptr).unwrap() != left;

        let mut next_step = if step_is_rev {
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