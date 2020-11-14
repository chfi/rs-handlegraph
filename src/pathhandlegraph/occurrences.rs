use super::path::PathId;

use super::StepUpdate;

use crate::handle::Handle;

pub trait OccurBase: Sized {
    type StepIx: Sized + Copy + Eq;
}

impl<'a, T: OccurBase> OccurBase for &'a T {
    type StepIx = T::StepIx;
}

impl<'a, T: OccurBase> OccurBase for &'a mut T {
    type StepIx = T::StepIx;
}

pub trait HandleOccurrences: OccurBase {
    type OccurIter: Iterator<Item = (PathId, Self::StepIx)>;

    fn handle_occurrences(self, handle: Handle) -> Self::OccurIter;
}

pub trait MutHandleOccurrences: OccurBase {
    fn apply_update(self, path_id: PathId, step: StepUpdate<Self::StepIx>);
}
