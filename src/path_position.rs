use crate::{
    handle::{Handle, NodeId},
    handlegraph::IntoSequences,
    packed::*,
    packedgraph::{index::OneBasedIndex, paths::StepPtr, PackedGraph},
    pathhandlegraph::{
        GraphPaths, GraphPathsSteps, IntoNodeOccurrences, IntoPathIds, PathId,
    },
};

use crate::packedgraph::graph::NARROW_PAGE_WIDTH;

use fnv::FnvHashMap;

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

    pub fn handle_positions(
        &self,
        graph: &PackedGraph,
        handle: Handle,
    ) -> Option<FnvHashMap<PathId, (StepPtr, usize)>> {
        let steps = graph.steps_on_handle(handle)?;

        let mut res: FnvHashMap<PathId, (StepPtr, usize)> =
            FnvHashMap::default();

        for (path_id, step_ix) in steps {
            let path = self.paths.get(path_id.0 as usize)?;
            let ix = step_ix.to_zero_based()?;
            let pos = path.step_positions.get(ix);

            res.insert(path_id, (step_ix, pos as usize));
        }

        Some(res)
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
