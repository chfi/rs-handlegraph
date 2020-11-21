use crate::handle::Handle;

use super::{
    MutPath, PathBase, PathId, PathStep, PathSteps, StepHandle, StepUpdate,
};
pub trait GraphPaths: Sized {
    type Step: PathStep;

    type StepIx: Sized + Copy + Eq;

    fn path_count(&self) -> usize;

    fn path_len(&self, id: PathId) -> Option<usize>;

    fn path_circular(&self, id: PathId) -> Option<bool>;

    fn path_step_at(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::Step>;

    fn path_first_step(&self, id: PathId) -> Option<Self::Step>;

    fn path_last_step(&self, id: PathId) -> Option<Self::Step>;

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step>;

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step>;
}

pub trait GraphPathNames {
    type PathName: Iterator<Item = u8>;

    fn get_path_id(&self, name: &[u8]) -> Option<PathId>;

    fn get_path_name(&self, id: PathId) -> Option<Self::PathName>;

    #[inline]
    fn get_path_name_vec(&self, id: PathId) -> Option<Vec<u8>> {
        self.get_path_name(id).map(|name| name.collect())
    }

    #[inline]
    fn has_path(&self, name: &[u8]) -> bool {
        self.get_path_id(name).is_some()
    }
}

pub trait IntoPathIds {
    type PathIds: Iterator<Item = PathId>;

    fn into_path_ids(self) -> Self::PathIds;
}

pub trait IntoNodeOccurrences: GraphPaths {
    type Occurrences: Iterator<Item = (PathId, Self::StepIx)>;

    fn into_steps_on_handle(self, handle: Handle) -> Self::Occurrences;
}

// pub trait MutableGraphPaths: GraphPaths + GraphPathNames {
pub trait MutableGraphPaths: GraphPaths {
    fn create_path(&mut self, name: &[u8]) -> Option<PathId>;

    fn destroy_path(&mut self, id: PathId) -> bool;

    fn path_append_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    fn path_prepend_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    fn path_insert_step_after(
        &mut self,
        id: PathId,
        index: Self::StepIx,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    fn path_remove_step(
        &mut self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx>;

    fn path_flip_step(
        &mut self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx>;

    fn path_rewrite_segment(
        &mut self,
        id: PathId,
        from: Self::StepIx,
        to: Self::StepIx,
        new_segment: &[Handle],
    ) -> Option<Vec<Self::StepIx>>;

    fn path_set_circularity(
        &mut self,
        id: PathId,
        circular: bool,
    ) -> Option<()>;
}

pub trait GraphPathsRef: GraphPaths {
    type PathRef: PathBase<Step = Self::Step, StepIx = Self::StepIx>;

    fn get_path_ref<'a>(&'a self, id: PathId) -> Option<&'a Self::PathRef>;
}

pub trait IntoPathSteps: GraphPathsRef
where
    Self::PathRef: PathSteps,
{
    fn into_path_steps(
        self,
        id: PathId,
    ) -> Option<<Self::PathRef as PathSteps>::Steps>;
}

pub trait GraphPathsRefMut: GraphPaths {
    type PathMut: PathBase<Step = Self::Step, StepIx = Self::StepIx>;

    fn get_path_mut_ref<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<&'a mut Self::PathMut>;
}

impl<'a, T> GraphPaths for &'a T
where
    T: GraphPaths,
{
    type Step = T::Step;
    type StepIx = T::StepIx;

    fn path_count(&self) -> usize {
        T::path_count(self)
    }

    fn path_len(&self, id: PathId) -> Option<usize> {
        T::path_len(self, id)
    }

    fn path_circular(&self, id: PathId) -> Option<bool> {
        T::path_circular(self, id)
    }

    fn path_step_at(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::Step> {
        T::path_step_at(self, id, index)
    }

    fn path_first_step(&self, id: PathId) -> Option<Self::Step> {
        T::path_first_step(self, id)
    }

    fn path_last_step(&self, id: PathId) -> Option<Self::Step> {
        T::path_last_step(self, id)
    }

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step> {
        T::path_next_step(self, id, step)
    }

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step> {
        T::path_prev_step(self, id, step)
    }
}

impl<'a, T> GraphPaths for &'a mut T
where
    T: GraphPaths,
{
    type Step = T::Step;
    type StepIx = T::StepIx;

    fn path_count(&self) -> usize {
        T::path_count(self)
    }

    fn path_len(&self, id: PathId) -> Option<usize> {
        T::path_len(self, id)
    }

    fn path_circular(&self, id: PathId) -> Option<bool> {
        T::path_circular(self, id)
    }

    fn path_step_at(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::Step> {
        T::path_step_at(self, id, index)
    }

    fn path_first_step(&self, id: PathId) -> Option<Self::Step> {
        T::path_first_step(self, id)
    }

    fn path_last_step(&self, id: PathId) -> Option<Self::Step> {
        T::path_last_step(self, id)
    }

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step> {
        T::path_next_step(self, id, step)
    }

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::Step,
    ) -> Option<Self::Step> {
        T::path_prev_step(self, id, step)
    }
}

pub trait AllPathIds: Sized {
    type PathIds: Iterator<Item = PathId>;

    fn all_path_ids(self) -> Self::PathIds;
}

pub trait PathNames: Sized {
    type PathName: Iterator<Item = u8>;

    fn get_path_name(self, id: PathId) -> Option<Self::PathName>;

    fn get_path_id(self, name: &[u8]) -> Option<PathId>;
}

pub trait PathNamesMut: Sized {
    fn insert_name(self, name: &[u8]) -> Option<PathId>;
}
