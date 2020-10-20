use bstr::BString;

use crate::handle::Handle;

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

    pub fn lookup_step_handle(&self, step: &PathStep) -> Option<Handle> {
        match step {
            PathStep::Front(_) => None,
            PathStep::End(_) => None,
            PathStep::Step(_, ix) => Some(self.nodes[*ix]),
        }
    }
}
