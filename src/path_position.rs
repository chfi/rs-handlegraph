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

        let mut path_ids = graph.path_ids().collect::<Vec<_>>();
        path_ids.sort();

        for path_id in path_ids {
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
    ) -> Option<Vec<(PathId, StepPtr, usize)>> {
        let steps = graph.steps_on_handle(handle)?;

        let mut res: Vec<(PathId, StepPtr, usize)> = Vec::new();

        for (path_id, step_ix) in steps {
            let path = self.paths.get(path_id.0 as usize)?;
            let ix = step_ix.to_zero_based()?;
            let pos = path.step_positions.get(ix);

            res.push((path_id, step_ix, pos as usize));
        }

        Some(res)
    }

    pub fn handle_positions_iter<'a>(
        &'a self,
        graph: &'a PackedGraph,
        handle: Handle,
    ) -> Option<impl Iterator<Item = (PathId, StepPtr, usize)> + 'a> {
        let steps = graph.steps_on_handle(handle)?;

        let paths = &self.paths;

        let iter = steps.filter_map(move |(path_id, step_ix)| {
            let path = paths.get(path_id.0 as usize)?;
            let ix = step_ix.to_zero_based()?;
            let pos = path.step_positions.get(ix);

            Some((path_id, step_ix, pos as usize))
        });

        Some(iter)
    }

    pub fn handles_positions<I>(
        &self,
        graph: &PackedGraph,
        handles: I,
    ) -> FnvHashMap<Handle, Vec<(PathId, StepPtr, usize)>>
    where
        I: Iterator<Item = Handle>,
    {
        let mut res: FnvHashMap<Handle, Vec<(PathId, StepPtr, usize)>> =
            FnvHashMap::default();

        let mut buf: Vec<(PathId, StepPtr, usize)> = Vec::new();

        for handle in handles {
            buf.clear();
            if let Some(positions) = self.handle_positions_iter(graph, handle) {
                buf.extend(positions);

                if !buf.is_empty() {
                    let mut pos_vec = buf.clone();
                    pos_vec.shrink_to_fit(); // not sure if needed
                    res.insert(handle, pos_vec);
                }
            }
        }

        res
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
