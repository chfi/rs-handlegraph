use std::num::NonZeroUsize;

use fnv::FnvHashMap;

use crossbeam_channel::Sender;

use crate::{
    handle::{Handle, NodeId},
    handlegraph::IntoSequences,
    packed::*,
    packedgraph::PackedGraph,
    pathhandlegraph::{
        GraphPaths, GraphPathsSteps, IntoPathIds, MutPath, PathBase, PathId,
        PathStep, PathSteps,
    },
};

use crate::packedgraph::{
    graph::NARROW_PAGE_WIDTH,
    index::list::{self, PackedDoubleList, PackedList, PackedListMut},
    paths::packedpath::{PackedStep, StepList},
};

pub struct PathPositionMap {
    pub(crate) paths: Vec<PathPositionIndex>,
}

impl PathPositionMap {
    pub fn index_paths(graph: &PackedGraph) -> Self {
        let mut paths: Vec<PathPositionIndex> =
            Vec::with_capacity(graph.path_count());

        for path_id in graph.path_ids() {
            let mut path_index = PathPositionIndex::default();

            let mut pos_offset = 0usize;

            if let Some(steps) = graph.path_steps(path_id) {
                for (_step_ix, step) in steps {
                    let seq_len = graph.sequence(step.handle).count();

                    path_index.step_positions.append(pos_offset as u64);

                    pos_offset += seq_len;
                }
            }

            paths.push(path_index);
        }

        Self { paths }
    }
}

pub struct PathPositionIndex {
    pub(crate) step_positions: RobustPagedIntVec,
}

impl std::default::Default for PathPositionIndex {
    fn default() -> Self {
        Self {
            step_positions: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
        }
    }
}
