use bstr::BString;
use fnv::FnvHashMap;

use crate::handle::{Handle, NodeId};

use crate::pathhandlegraph::{PathBase, PathRef, PathRefMut};

use super::Node;

pub type PathId = i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathStep {
    Front(i64),
    End(i64),
    Step(i64, usize),
}

impl PathStep {
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

#[derive(Debug)]
pub struct Path {
    pub path_id: PathId,
    pub name: BString,
    pub is_circular: bool,
    pub nodes: Vec<Handle>,
}

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

    pub fn lookup_step_handle(&self, step: &PathStep) -> Option<Handle> {
        match step {
            PathStep::Front(_) => None,
            PathStep::End(_) => None,
            PathStep::Step(_, ix) => Some(self.nodes[*ix]),
        }
    }

    pub fn position_of_step(
        &self,
        graph: &FnvHashMap<NodeId, Node>,
        step: &PathStep,
    ) -> Option<usize> {
        if step.path_id() != self.path_id {
            return None;
        }

        match step {
            PathStep::Front(_) => Some(0),
            PathStep::End(_) => Some(self.bases_len(graph)),
            &PathStep::Step(_, step_ix) => {
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
    ) -> PathStep {
        if pos == 0 {
            return PathStep::Front(self.path_id);
        }

        let mut bases = 0;
        for (ix, handle) in self.nodes.iter().enumerate() {
            let node = graph.get(&handle.id()).unwrap();
            bases += node.sequence.len();
            if pos < bases {
                return PathStep::Step(self.path_id, ix);
            }
        }

        PathStep::End(self.path_id)
    }
}
