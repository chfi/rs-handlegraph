/*!

`HashGraph` paths, and steps on paths


*/

#![allow(dead_code)]
use fnv::FnvHashMap;

use crate::handle::{Handle, NodeId};

use crate::pathhandlegraph::{PathBase, PathId, PathStep, PathSteps};

use super::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepIx {
    Front,
    End,
    Step(usize),
}

impl StepIx {
    pub fn index(&self) -> Option<usize> {
        if let Self::Step(ix) = self {
            Some(*ix)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step(pub StepIx, pub Handle);

impl PathStep for Step {
    fn handle(&self) -> Handle {
        self.1
    }
}

#[derive(Debug)]
pub struct Path {
    pub path_id: PathId,
    pub name: Vec<u8>,
    pub is_circular: bool,
    pub nodes: Vec<Handle>,
}

impl PathBase for Path {
    type Step = Step;

    type StepIx = StepIx;

    #[inline]
    fn len(&self) -> usize {
        self.nodes.len()
    }

    #[inline]
    fn circular(&self) -> bool {
        self.is_circular
    }

    #[inline]
    fn step_at(&self, index: StepIx) -> Option<Step> {
        let handle = self.lookup_step_handle(&index)?;
        Some(Step(index, handle))
    }

    #[inline]
    fn first_step(&self) -> StepIx {
        StepIx::Step(0)
    }

    #[inline]
    fn last_step(&self) -> StepIx {
        StepIx::Step(self.nodes.len() - 1)
    }

    #[inline]
    fn next_step(&self, step: StepIx) -> Option<Step> {
        let next_ix = match step {
            StepIx::Front => Some(0),
            StepIx::End => None,
            StepIx::Step(ix) => {
                let len = self.nodes.len();
                if ix < len - 1 {
                    Some(ix + 1)
                } else {
                    None
                }
            }
        }?;

        let handle = self.nodes.get(next_ix)?;
        Some(Step(StepIx::Step(next_ix), *handle))
    }

    #[inline]
    fn prev_step(&self, step: StepIx) -> Option<Step> {
        let prev_ix = match step {
            StepIx::Front => None,
            StepIx::End => Some(self.nodes.len() - 1),
            StepIx::Step(ix) => {
                if ix > 0 {
                    Some(ix - 1)
                } else {
                    None
                }
            }
        }?;

        let handle = self.nodes.get(prev_ix)?;
        Some(Step(StepIx::Step(prev_ix), *handle))
    }
}

impl<'a> PathSteps for &'a Path {
    type Steps = StepsIter<'a>;

    fn steps(self) -> Self::Steps {
        StepsIter::new(&self.nodes)
    }
}

pub struct StepsIter<'a> {
    nodes: &'a [Handle],
    left: usize,
    right: usize,
    finished: bool,
}

impl<'a> StepsIter<'a> {
    fn new(nodes: &'a [Handle]) -> Self {
        let left = 0;
        let right = nodes.len() - 1;
        let finished = false;
        Self {
            nodes,
            left,
            right,
            finished,
        }
    }
}

impl<'a> Iterator for StepsIter<'a> {
    type Item = Step;

    #[inline]
    fn next(&mut self) -> Option<Step> {
        if self.finished {
            return None;
        }

        let handle = *self.nodes.get(self.left)?;
        let index = StepIx::Step(self.left);

        self.left += 1;
        if self.left > self.right {
            self.finished = true;
        }

        Some(Step(index, handle))
    }
}

impl<'a> DoubleEndedIterator for StepsIter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Step> {
        if self.finished {
            return None;
        }

        let handle = *self.nodes.get(self.right)?;
        let index = StepIx::Step(self.right);

        self.right -= 1;
        if self.left > self.right {
            self.finished = true;
        }

        Some(Step(index, handle))
    }
}

impl Path {
    pub fn new(name: &[u8], path_id: PathId, is_circular: bool) -> Self {
        Path {
            name: name.into(),
            path_id,
            is_circular,
            nodes: vec![],
        }
    }

    pub fn step_index_offset(&self, step: StepIx) -> usize {
        match step {
            StepIx::Front => 0,
            StepIx::End => self.nodes.len() - 1,
            StepIx::Step(i) => i,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn bases_len(&self, graph: &FnvHashMap<NodeId, Node>) -> usize {
        self.nodes
            .iter()
            .filter_map(|handle| {
                graph.get(&handle.id()).map(|n| n.sequence.len())
            })
            .sum()
    }

    pub fn lookup_step_handle(&self, step: &StepIx) -> Option<Handle> {
        match step {
            StepIx::Front => None,
            StepIx::End => None,
            StepIx::Step(ix) => Some(self.nodes[*ix]),
        }
    }

    pub fn position_of_step(
        &self,
        graph: &FnvHashMap<NodeId, Node>,
        step: StepIx,
    ) -> Option<usize> {
        match step {
            StepIx::Front => Some(0),
            StepIx::End => Some(self.bases_len(graph)),
            StepIx::Step(step_ix) => {
                let mut bases = 0;
                for handle in self.nodes[0..step_ix].iter() {
                    let node = graph.get(&handle.id())?;
                    bases += node.sequence.len();
                }
                Some(bases)
            }
        }
    }

    pub fn step_at_position(
        &self,
        graph: &FnvHashMap<NodeId, Node>,
        pos: usize,
    ) -> StepIx {
        if pos == 0 {
            return StepIx::Front;
        }

        let mut bases = 0;
        for (ix, handle) in self.nodes.iter().enumerate() {
            let node = graph.get(&handle.id()).unwrap();
            bases += node.sequence.len();
            if pos < bases {
                return StepIx::Step(ix);
            }
        }

        StepIx::End
    }
}
