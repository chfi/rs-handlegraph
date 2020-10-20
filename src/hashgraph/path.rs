use bstr::BString;
use std::collections::HashMap;

use crate::handle::{Handle, NodeId};

use super::Node;

pub type PathId = i64;

#[derive(Debug, Clone, PartialEq)]
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

    pub fn bases_len(&self, graph: &HashMap<NodeId, Node>) -> usize {
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
        graph: &HashMap<NodeId, Node>,
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

    pub fn step_at_position(&self, pos: usize) -> PathStep {
        if pos == 0 {
            return PathStep::Front(*self.path_id);
        }

        let mut bases = 0;
        for (ix, handle) in self.nodes.iter().enumerate() {
            let node = graph.get(&handle.id()).unwrap();
            bases += node.sequence.len();
            if pos < bases {
                return PathStep::Step(*self.path_id, ix);
            }
        }

        return PathStep::End(*self.path_id);
    }
}
