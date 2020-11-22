use crate::handle::Handle;

use super::{
    MutPath, PathBase, PathId, PathStep, PathSteps, StepHandle, StepUpdate,
};

/// A handlegraph with embedded paths. The step for any given path is
/// indexed by the associated type `StepIx`.
///
/// Provides methods for basic querying of the graph's paths, and
/// steps on a path. For a more ergonomic way of iterating through the
/// steps of a path, see the traits `GraphPathsRef`, and `PathSteps`.
pub trait GraphPaths: Sized {
    type StepIx: Sized + Copy + Eq;

    /// Return the number of paths in this graph.
    fn path_count(&self) -> usize;

    /// Return the number of steps of the path `id`, if it exists.
    fn path_len(&self, id: PathId) -> Option<usize>;

    /// Return the circularity of the path `id`, if it exists.
    fn path_circular(&self, id: PathId) -> Option<bool>;

    /// Find the handle at step `index` in path `id`, if both the path
    /// exists, and the step in the path.
    fn path_handle_at_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Handle>;

    /// Return the index to the first step of path `id`, if the path
    /// exists and is not empty.
    ///
    /// The resulting `StepIx` should point to the first step of the
    /// path's `Steps` iterator.
    fn path_first_step(&self, id: PathId) -> Option<Self::StepIx>;

    /// Return the index to the last step of path `id`, if the path
    /// exists and is not empty.
    ///
    /// The resulting `StepIx` should point to the last step of the
    /// path's `Steps` iterator.
    fn path_last_step(&self, id: PathId) -> Option<Self::StepIx>;

    /// Return the index to the step after `index` on path `id`, if
    /// the path exists and `index` both exists on the path, and is
    /// not the last step of the path.
    ///
    /// The resulting `StepIx` should point to the same step as would
    /// calling `next` on the path's corresponding `Steps` iterator,
    /// if `index` was the last produced step.
    fn path_next_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx>;

    /// Return the index to the step before `index` on path `id`, if
    /// the path exists and `index` both exists on the path, and is
    /// not the first step of the path.
    ///
    /// The resulting `StepIx` should point to the same step as would
    /// calling `next_back` on the path's corresponding `Steps` iterator,
    /// if `index` was the last produced step.
    fn path_prev_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx>;
}

/// Trait for retrieving the `PathId` for a path by name, and vice
/// versa.
///
/// Names are represented as an iterator over `u8`s for flexibility in
/// underlying storage.
pub trait GraphPathNames: Sized {
    /// The iterator on the name of a path.
    type PathName: Iterator<Item = u8>;

    /// Returns the `PathId` that the provided `name` points to, if
    /// there exists a path with that name.
    fn get_path_id(self, name: &[u8]) -> Option<PathId>;

    /// Returns an iterator that produced the name of the path `id`,
    /// if that path exists in the graph.
    fn get_path_name(self, id: PathId) -> Option<Self::PathName>;

    /// Convenience method for retrieving a path name as a `Vec<u8>`.
    #[inline]
    fn get_path_name_vec(self, id: PathId) -> Option<Vec<u8>> {
        self.get_path_name(id).map(|name| name.collect())
    }

    /// Convenience method for checking whether a path exists by name.
    #[inline]
    fn has_path(self, name: &[u8]) -> bool {
        self.get_path_id(name).is_some()
    }
}

pub trait IntoPathIds {
    type PathIds: Iterator<Item = PathId>;

    fn into_path_ids(self) -> Self::PathIds;
}

pub trait IntoNodeOccurrences: GraphPaths {
    type Occurrences: Iterator<Item = (PathId, Self::StepIx)>;

    fn into_steps_on_handle(self, handle: Handle) -> Option<Self::Occurrences>;
}

pub trait MutableGraphPaths: GraphPaths {
    fn create_path(&mut self, name: &[u8], circular: bool) -> Option<PathId>;

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
    ) -> Option<(Self::StepIx, Self::StepIx)>;

    fn path_set_circularity(
        &mut self,
        id: PathId,
        circular: bool,
    ) -> Option<()>;
}

pub trait PathSequences: GraphPaths {
    fn path_bases_len(&self, id: PathId) -> Option<usize>;

    fn path_step_at_base(&self, id: PathId, pos: usize)
        -> Option<Self::StepIx>;

    fn path_step_base_offset(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<usize>;
}

pub trait GraphPathsRef: GraphPaths {
    type PathRef: PathBase<StepIx = Self::StepIx>;

    fn get_path_ref(self, id: PathId) -> Option<Self::PathRef>;
}

pub trait GraphPathsRefMut: GraphPaths {
    type PathMut: PathBase<StepIx = Self::StepIx>;

    fn get_path_mut_ref<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<&'a mut Self::PathMut>;
}

impl<'a, T> GraphPaths for &'a T
where
    T: GraphPaths,
{
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

    fn path_handle_at_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Handle> {
        T::path_handle_at_step(self, id, index)
    }

    fn path_first_step(&self, id: PathId) -> Option<Self::StepIx> {
        T::path_first_step(self, id)
    }

    fn path_last_step(&self, id: PathId) -> Option<Self::StepIx> {
        T::path_last_step(self, id)
    }

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        T::path_next_step(self, id, step)
    }

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        T::path_prev_step(self, id, step)
    }
}

impl<'a, T> GraphPaths for &'a mut T
where
    T: GraphPaths,
{
    // type Step = T::Step;
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

    fn path_handle_at_step(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Handle> {
        T::path_handle_at_step(self, id, index)
    }

    fn path_first_step(&self, id: PathId) -> Option<Self::StepIx> {
        T::path_first_step(self, id)
    }

    fn path_last_step(&self, id: PathId) -> Option<Self::StepIx> {
        T::path_last_step(self, id)
    }

    fn path_next_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        T::path_next_step(self, id, step)
    }

    fn path_prev_step(
        &self,
        id: PathId,
        step: Self::StepIx,
    ) -> Option<Self::StepIx> {
        T::path_prev_step(self, id, step)
    }
}
