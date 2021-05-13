use crate::{
    handle::Handle,
    handlegraph::IntoSequences,
    packed::*,
    packedgraph::{index::OneBasedIndex, paths::StepPtr, PackedGraph},
    pathhandlegraph::{
        GraphPaths, GraphPathsSteps, IntoNodeOccurrences, IntoPathIds, PathId,
    },
};

use crate::packedgraph::graph::NARROW_PAGE_WIDTH;

use fnv::FnvHashMap;

#[derive(Debug, Clone)]
pub struct PathPositionMap {
    pub(crate) paths: Vec<PathPositionIndex>,
}

impl PathPositionMap {
    /// Build a `PathPositionMap` index from a `PackedGraph`.
    ///
    /// IMPORTANT: Assumes that each path in the graph has been
    /// constructed by appending steps in order, and that no steps
    /// have been deleted!
    pub fn index_paths(graph: &PackedGraph) -> Self {
        let mut paths: Vec<PathPositionIndex> =
            Vec::with_capacity(graph.path_count());

        let mut path_ids = graph.path_ids().collect::<Vec<_>>();
        path_ids.sort();

        for path_id in path_ids {
            let mut path_index = PathPositionIndex::default();

            let mut pos_offset = 1usize;
            let mut last_step_offset = 0usize;

            if let Some(steps) = graph.path_steps(path_id) {
                for (_step_ix, step) in steps {
                    let seq_len = graph.sequence(step.handle).count();

                    path_index.step_positions.append(pos_offset as u64);

                    last_step_offset = pos_offset;
                    pos_offset += seq_len;
                }
            }

            path_index.last_step_offset = last_step_offset;
            path_index.base_len = pos_offset;

            paths.push(path_index);
        }

        Self { paths }
    }

    pub fn path_base_len(&self, path: PathId) -> Option<usize> {
        let path_indices = self.paths.get(path.0 as usize)?;
        Some(path_indices.base_len)
    }

    pub fn path_step_position(
        &self,
        path: PathId,
        step: StepPtr,
    ) -> Option<usize> {
        let path_indices = self.paths.get(path.0 as usize)?;

        let step_ix = step.to_zero_based()?;

        Some(path_indices.step_positions.get(step_ix) as usize)
    }

    pub fn find_step_at_base(
        &self,
        path: PathId,
        base_pos: usize,
    ) -> Option<StepPtr> {
        let path_indices = self.paths.get(path.0 as usize)?;

        if base_pos >= path_indices.last_step_offset {
            if base_pos > path_indices.base_len {
                return None;
            } else {
                return Some(StepPtr::from_zero_based(
                    path_indices.step_positions.len(),
                ));
            }
        }

        let step_count = path_indices.step_positions.len();

        let steps = &path_indices.step_positions;

        let mut left = 0;
        let mut right = step_count - 1;
        let mut mid;

        loop {
            if left > right {
                return None;
            }

            mid = (left + right) / 2;

            let at_mid = steps.get(mid) as usize;
            let at_next = if mid == step_count {
                path_indices.base_len
            } else {
                steps.get(mid + 1) as usize
            };

            if at_mid < base_pos {
                left = mid + 1;
            } else if at_next > base_pos {
                right = mid - 1;
            } else {
                break;
            }
        }

        Some(StepPtr::from_zero_based(mid))
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

#[derive(Debug, Clone)]
pub struct PathPositionIndex {
    pub(crate) step_positions: RobustPagedIntVec,
    pub(crate) base_len: usize,
    pub(crate) last_step_offset: usize,
}

impl std::default::Default for PathPositionIndex {
    fn default() -> Self {
        Self {
            step_positions: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            base_len: 0usize,
            last_step_offset: 0usize,
        }
    }
}
