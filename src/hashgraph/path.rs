#![allow(dead_code)]

use bstr::BString;
use fnv::FnvHashMap;

use crate::handle::{Handle, NodeId};

use crate::pathhandlegraph::{PathBase, PathId, PathStep, PathSteps};

use super::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepIx {
    Front(PathId),
    End(PathId),
    Step(PathId, usize),
}

impl StepIx {
    pub fn index(&self) -> Option<usize> {
        if let Self::Step(_, ix) = self {
            Some(*ix)
        } else {
            None
        }
    }

    pub fn path_id(&self) -> PathId {
        match self {
            Self::Front(i) => *i,
            Self::End(i) => *i,
            Self::Step(i, _) => *i,
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
    pub name: BString,
    pub is_circular: bool,
    pub nodes: Vec<Handle>,
}

/*
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
        let step = self.path.steps_ref().get_step(index)?;
        Some((index, step))
    }

    #[inline]
    fn first_step(&self) -> Step {
        let head = self.head;
        let step = self.path.steps_ref().get_step_unchecked(head);
        (head, step)
    }

    #[inline]
    fn last_step(&self) -> Step {
        let tail = self.tail;
        let step = self.path.steps_ref().get_step_unchecked(tail);
        (tail, step)
    }

    #[inline]
    fn next_step(&self, step: Step) -> Option<Step> {
        let next = self.path.steps_ref().next_step(step.0)?;
        let next_step = self.path.steps_ref().get_step_unchecked(next);
        Some((next, next_step))
    }

    #[inline]
    fn prev_step(&self, step: Step) -> Option<Step> {
        let prev = self.path.steps_ref().prev_step(step.0)?;
        let prev_step = self.path.steps_ref().get_step_unchecked(prev);
        Some((prev, prev_step))
    }
}
*/

// impl<'a> PathSteps for &'a Path {
//     type Steps = stD::slice::
// }

/*
pub struct PathStepIter<'a> {
    handles: std::slice::Iter<'a, Handle>,
    index: usize,
}

impl<'a> PathStepIter<'a> {
    fn new(nodes: &'a [Handle]) -> PathStepIter<'a> {
        let handles = nodes.iter();
        Self { handles, index: 0 }
    }
}

impl<'a> Iterator for PathStepIter<'a> {
    type Item = crate::pathhandlegraph::PathStep;

    fn next(&mut self) -> Option<Self::Item> {
        let _handle = self.handles.next()?;
        let ix = self.index;
        let item = crate::pathhandlegraph::PathStep::Step(ix);
        self.index += 1;
        Some(item)
    }
}
*/

/*
use crate::pathhandlegraph::PathStep as PStep;

impl PathBase for Path {
    type Step = PStep;
}

impl<'a> PathRef for &'a Path {
    type Steps = PathStepIter<'a>;

    fn steps(self) -> Self::Steps {
        PathStepIter::new(&self.nodes)
    }

    fn len(self) -> usize {
        self.nodes.len()
    }

    fn circular(self) -> bool {
        self.is_circular
    }

    fn handle_at(self, step: PStep) -> Option<Handle> {
        if let PStep::Step(ix) = step {
            self.nodes.get(ix).copied()
        } else {
            None
        }
    }

    fn contains(self, handle: Handle) -> bool {
        self.nodes.contains(&handle)
    }

    // fn next_step(self, step: PStep) -> Option<PStep> {
}

impl<'a> PathRefMut for &'a mut Path {
    fn append(self, handle: Handle) -> PStep {
        let new_step = PStep::Step(self.nodes.len());
        self.nodes.push(handle);
        new_step
    }

    fn prepend(self, handle: Handle) -> PStep {
        let new_step = PStep::Step(0);
        self.nodes.insert(0, handle);
        new_step
    }

    fn set_circularity(self, circular: bool) {
        self.is_circular = circular;
    }
}
*/

impl Path {
    pub fn new<T: Into<BString>>(
        name: T,
        path_id: PathId,
        is_circular: bool,
    ) -> Self {
        Path {
            name: name.into(),
            path_id,
            is_circular,
            nodes: vec![],
        }
    }

    pub fn step_index_offset(&self, step: StepIx) -> usize {
        match step {
            StepIx::Front(_) => 0,
            StepIx::End(_) => self.nodes.len() - 1,
            StepIx::Step(_, i) => i,
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
            StepIx::Front(_) => None,
            StepIx::End(_) => None,
            StepIx::Step(_, ix) => Some(self.nodes[*ix]),
        }
    }

    pub fn position_of_step(
        &self,
        graph: &FnvHashMap<NodeId, Node>,
        step: &StepIx,
    ) -> Option<usize> {
        if step.path_id() != self.path_id {
            return None;
        }

        match step {
            StepIx::Front(_) => Some(0),
            StepIx::End(_) => Some(self.bases_len(graph)),
            &StepIx::Step(_, step_ix) => {
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
            return StepIx::Front(self.path_id);
        }

        let mut bases = 0;
        for (ix, handle) in self.nodes.iter().enumerate() {
            let node = graph.get(&handle.id()).unwrap();
            bases += node.sequence.len();
            if pos < bases {
                return StepIx::Step(self.path_id, ix);
            }
        }

        StepIx::End(self.path_id)
    }
}
