/*!

Traits that cover interacting with and manipulating paths in a graph.

* [`GraphPaths`] has the basic interface
* [`GraphPathNames`] supports retrieving the ID of a path by name, or vice versa
* [`MutableGraphPaths`] includes creating and destroying paths, and methods for manipulating the steps on a path
* [`PathSequences`] is for going between path step indices and sequence positions
* [`GraphPathsRef`] provides a reference to a specific path, which can then be queried using the traits in [`super::path`]
* [`IntoPathIds`] provides an iterator on the paths by ID
* [`IntoNodeOccurrences`] provides an iterator on the steps that are on a given node

*/

use crate::handle::Handle;

use super::{PathBase, PathId};

/// Trait for iterating through all `PathIds` in a graph.
pub trait IntoPathIds {
    type PathIds: Iterator<Item = PathId>;

    fn path_ids(self) -> Self::PathIds;
}

/// Trait for iterating through all the path steps on a handle in a graph.
pub trait IntoNodeOccurrences: GraphPaths {
    /// An iterator through the steps on a path, by `PathId` and `StepIx`.
    type Occurrences: Iterator<Item = (PathId, Self::StepIx)>;

    fn steps_on_handle(self, handle: Handle) -> Option<Self::Occurrences>;
}

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

/// A handlegraph with embedded paths that can be created, destroyed,
/// and otherwise manipulated.
pub trait MutableGraphPaths: GraphPaths {
    /// Create a new path with the given name and return its `PathId`.
    /// Returns `None` if the path already exists in the graph.
    fn create_path(&mut self, name: &[u8], circular: bool) -> Option<PathId>;

    /// Destroy the path with the given `id`. Returns `true` if the
    /// path was destroyed, `false` if the path did not exist or
    /// couldn't be destroyed.
    fn destroy_path(&mut self, id: PathId) -> bool;

    /// Append a step on the given `handle` to the end of path `id`,
    /// if the path exists. Returns the index of the new step.
    fn path_append_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    /// Prepend a step on the given `handle` to the beginning of path
    /// `id`, if the path exists. Returns the index of the new step.
    fn path_prepend_step(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    /// Insert a step on the given `handle` into path `id`, after the
    /// step at `index`. Returns the index of the new step if it was
    /// successfully inserted, or `None` if either the path or the
    /// step does not exist.
    fn path_insert_step_after(
        &mut self,
        id: PathId,
        index: Self::StepIx,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    /// Remove the step at `index` from path `id`. Returns the index
    /// of the removed step if it existed and was removed.
    fn path_remove_step(
        &mut self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx>;

    /// Flip the orientation of the handle on step at `index` on path
    /// `id`, if it exists.
    fn path_flip_step(
        &mut self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<Self::StepIx>;

    /// Replace the steps starting from the step at `from` (inclusive)
    /// until the step `to` (exclusive) with steps on the `Handle`s in
    /// `new_segment`. Returns a pair where the first entry is the
    /// pointer to the step corresponding to the first handle in
    /// `new_segment`, and the second entry, the step corresponding to
    /// the last handle.
    ///
    /// Depending on the graph implementation, if `to` denotes a step
    /// beyond the path, all steps beginning at `from` will be removed
    /// and replaced. If `new_segment` is empty, the range will simply
    /// be deleted, contracting the path. In that case, which pointers
    /// are returned depend on the implementation.
    ///
    /// The step `from` must come before `to` in the path, but it's up
    /// to implementations to choose how to handle it if that's not
    /// the case -- potentially panicking.
    fn path_rewrite_segment(
        &mut self,
        id: PathId,
        from: Self::StepIx,
        to: Self::StepIx,
        new_segment: &[Handle],
    ) -> Option<(Self::StepIx, Self::StepIx)>;

    /// Set the circularity of path `id`.
    fn path_set_circularity(
        &mut self,
        id: PathId,
        circular: bool,
    ) -> Option<()>;
}

/// A handlegraph with embedded paths whose steps are associated with
/// the sequence positions and lengths of their nodes.
pub trait PathSequences: GraphPaths {
    /// Return the length of path `id` in nucleotides, if it exists.
    fn path_bases_len(&self, id: PathId) -> Option<usize>;

    /// Return the index of the step at sequence position `pos` along
    /// path `id`, or `None` if either the path doesn't exist, or if
    /// the path is shorter than `pos` bases.
    fn path_step_at_base(&self, id: PathId, pos: usize)
        -> Option<Self::StepIx>;

    /// Return the sequence offset of the step at `index` in path
    /// `id`, if it exists.
    fn path_step_base_offset(
        &self,
        id: PathId,
        index: Self::StepIx,
    ) -> Option<usize>;
}

/// A handlegraph that can produce references to specific paths.
pub trait GraphPathsRef: GraphPaths {
    type PathRef: PathBase<StepIx = Self::StepIx>;
    fn get_path_ref(self, id: PathId) -> Option<Self::PathRef>;
}

pub trait GraphPathsSteps: GraphPathsRef {
    type Step: super::path::PathStep;
    type Steps: DoubleEndedIterator<Item = Self::Step>;

    fn path_steps(self, id: PathId) -> Option<Self::Steps>;

    fn path_steps_range(
        self,
        id: PathId,
        from: Self::StepIx,
        to: Self::StepIx,
    ) -> Option<Self::Steps>;
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
