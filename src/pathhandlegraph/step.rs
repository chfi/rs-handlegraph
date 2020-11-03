use super::path::PathId;

/// A step along a path; the path context is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathStep {
    Before,
    After,
    Step(usize),
}

impl PathStep {
    #[inline]
    pub fn is_before(&self) -> bool {
        *self == PathStep::Before
    }

    #[inline]
    pub fn is_after(&self) -> bool {
        *self == PathStep::After
    }

    #[inline]
    pub fn index(&self) -> Option<usize> {
        if let PathStep::Step(ix) = *self {
            Some(ix)
        } else {
            None
        }
    }
}

/// A step along a specific path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StepHandle {
    path: PathId,
    step: PathStep,
}
