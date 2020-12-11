/*!
Traits and types to use when working with a single specific path in a
graph.

For simple queries and manipulation of embedded paths, the traits in
[`embedded_paths`](crate::pathhandlegraph::embedded_paths) may be good
enough.

* [`PathBase`] defines the fundamental path interface, and provides a simple interface for working with steps on a path
* [`PathSteps`] extends `PathBase` with an iterator on the path's steps

The [`MutPath`] trait is more experimental, and is unlikely to be used
directly.

*/

use crate::handle::Handle;

/// A unique identifier for a single path.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PathId(pub u64);

crate::impl_space_usage_stack_newtype!(PathId);

impl crate::packed::PackedElement for PathId {
    #[inline]
    fn unpack(v: u64) -> Self {
        PathId(v)
    }

    #[inline]
    fn pack(self) -> u64 {
        self.0
    }
}

/// Anything that holds a `Handle` can be a step on a path
pub trait PathStep: Sized + Copy + Eq {
    fn handle(&self) -> Handle;
}

/// The base trait for any path that a handlegraph with embedded paths
/// can contain.
///
/// Defines the type used to index steps on the path, and the type of
/// the steps themselves. Provides an interface for querying the
/// path's properties, and individual steps, including retrieving
/// adjacent steps.
///
/// There's a blanket implementation of `PathBase` for both shared and
/// mutable references of all implementors of `PathBase`.
pub trait PathBase: Sized {
    /// A step on the path. The `PathStep` constraint ensures that a
    /// step in some way contains a handle.
    type Step: PathStep;

    /// An index to a step on the path.
    type StepIx: Sized + Copy + Eq;

    /// The number of steps on the path.
    fn len(&self) -> usize;

    /// True if the path contains no steps.
    #[inline]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// True if the path is circular.
    fn circular(&self) -> bool;

    /// Return the `Step` at the provided `index`, if the index
    /// actually points to a step on this path.
    fn step_at(&self, index: Self::StepIx) -> Option<Self::Step>;

    /// Return the first step of the path.
    fn first_step(&self) -> Self::StepIx;

    /// Return the last step of the path.
    fn last_step(&self) -> Self::StepIx;

    /// Return the step after the provided `step`, if `step` is not
    /// the last step on the path.
    fn next_step(&self, step: Self::StepIx) -> Option<Self::Step>;

    /// Return the step before the provided `step`, if `step` is not
    /// the first step on the path.
    fn prev_step(&self, step: Self::StepIx) -> Option<Self::Step>;
}

/// A path that provides an iterator through its steps in both
/// directions.
pub trait PathSteps: PathBase {
    type Steps: DoubleEndedIterator<Item = Self::Step>;

    fn steps(self) -> Self::Steps;

    // Return an iterator on the path's steps starting from the step
    // at `index`. Returns `None` if `index` is not on the path.

    // fn steps_from(self, index: Self::StepIx) -> Option<Self::Steps>;

    // Return an iterator on the path's steps from the start, until
    // the step at `index`. If `index` is not a step on the path, the
    // iterator will produce all steps.

    // fn steps_until(self, index: Self::StepIx) -> Self::Steps;

    // Return an iterator on the steps between `from` and `to`, both
    // inclusive. Returns `None` if `from` does not exist in the path.
    // If `to` does not exist in the path, the iterator will continue
    // until the end of the path.

    // fn steps_in_range(self, from: Self::StepIx, to: Self::StepIx) -> Option<Self::Steps>;

    /// `true` if one of the path's steps is on `handle`.
    fn contains(self, handle: Handle) -> bool {
        self.steps().any(|s| s.handle() == handle)
    }
}

/// An embedded path that can also be mutated by appending or
/// prepending steps, or rewriting parts of it.
///
/// This is not intended to be used for manipulating paths in general,
/// which is why the functions return
/// [`StepUpdate`s](super::StepUpdate). Instead, this can be used as a
/// simple "DSL" for path manipulation in a lazy way, producing
/// operations that can be efficiently applied in a batched (and
/// possibly parallel) manner.
///
/// See the impl on
/// [`PackedPath`](crate::packedgraph::paths::packedpath::PackedPath) for
/// an example on how it can be used.
pub trait MutPath: PathBase {
    /// Extend the path by append a step on `handle` to the end,
    /// returning a `StepUpdate` that includes the new step index.
    fn append_step(&mut self, handle: Handle) -> StepUpdate<Self::StepIx>;

    fn append_steps_iter<I>(
        &mut self,
        iter: I,
    ) -> Vec<StepUpdate<Self::StepIx>>
    where
        I: Iterator<Item = Handle>;

    /// Extend the path by prepend a step on `handle` before the
    /// beginning, returning a `StepUpdate` that includes the new step
    /// index.
    fn prepend_step(&mut self, handle: Handle) -> StepUpdate<Self::StepIx>;

    /// Insert a step with `handle` after the step at `index`,
    /// returning the `StepUpdate` corresponding to the new step.
    /// Returns `None` if `index` does not point to a step in this path.
    fn insert_step_after(
        &mut self,
        index: Self::StepIx,
        handle: Handle,
    ) -> Option<StepUpdate<Self::StepIx>>;

    /// Remove the step at `index` from this path if it exists, and
    /// return the corresponding `StepUpdate`.
    fn remove_step(
        &mut self,
        index: Self::StepIx,
    ) -> Option<StepUpdate<Self::StepIx>>;

    /// Remove all steps between the indices `from` and `to`
    /// (inclusive), and insert new steps on the `Handle`s in
    /// `new_segment`. Returns a tuple with the step indices of the
    /// first and last new handles, and the corresponding
    /// `StepUpdate`s.
    fn rewrite_segment(
        &mut self,
        from: Self::StepIx,
        to: Self::StepIx,
        new_segment: &[Handle],
    ) -> Option<(Self::StepIx, Self::StepIx, Vec<StepUpdate<Self::StepIx>>)>;

    /// Set the circularity of the path to the provided `circular`.
    fn set_circularity(&mut self, circular: bool);

    /// Flip the handle orientation of the step at `index`, if it
    /// exists. Returns the corresponding `StepUpdates` that remove
    /// the handle with the original orientation, and inserts the
    /// handle with the flipped orientation at `index`.
    fn flip_step(
        &mut self,
        index: Self::StepIx,
    ) -> Option<Vec<StepUpdate<Self::StepIx>>>;
}

/// Defines a change to a path that can be used update the occurrence
/// records of a node in a graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepUpdate<StepIx: Sized + Copy + Eq> {
    Insert { handle: Handle, step: StepIx },
    Remove { handle: Handle, step: StepIx },
    // InsertSegment {
    //     start: StepIx,
    //     steps: Vec<(Handle, StepIx)>,
    // },
}

impl<StepIx: Sized + Copy + Eq> StepUpdate<StepIx> {
    pub fn handle(&self) -> Handle {
        match self {
            StepUpdate::Insert { handle, .. } => *handle,
            StepUpdate::Remove { handle, .. } => *handle,
        }
    }

    pub fn step(&self) -> StepIx {
        match self {
            StepUpdate::Insert { step, .. } => *step,
            StepUpdate::Remove { step, .. } => *step,
        }
    }
}

/// Blanket implementation of `PathBase` for references of types that
/// implement `PathBase`.
impl<'a, T> PathBase for &'a T
where
    T: PathBase,
{
    type Step = T::Step;

    type StepIx = T::StepIx;

    #[inline]
    fn len(&self) -> usize {
        <T as PathBase>::len(self)
    }

    #[inline]
    fn circular(&self) -> bool {
        <T as PathBase>::circular(self)
    }

    #[inline]
    fn step_at(&self, index: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::step_at(self, index)
    }

    #[inline]
    fn first_step(&self) -> Self::StepIx {
        <T as PathBase>::first_step(self)
    }

    #[inline]
    fn last_step(&self) -> Self::StepIx {
        <T as PathBase>::last_step(self)
    }

    #[inline]
    fn next_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::next_step(self, step)
    }

    #[inline]
    fn prev_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::next_step(self, step)
    }
}

impl<'a, T> PathBase for &'a mut T
where
    T: PathBase,
{
    type Step = T::Step;

    type StepIx = T::StepIx;

    #[inline]
    fn len(&self) -> usize {
        <T as PathBase>::len(self)
    }

    #[inline]
    fn circular(&self) -> bool {
        <T as PathBase>::circular(self)
    }

    #[inline]
    fn step_at(&self, index: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::step_at(self, index)
    }

    #[inline]
    fn first_step(&self) -> Self::StepIx {
        <T as PathBase>::first_step(self)
    }

    #[inline]
    fn last_step(&self) -> Self::StepIx {
        <T as PathBase>::last_step(self)
    }

    #[inline]
    fn next_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::next_step(self, step)
    }

    #[inline]
    fn prev_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        <T as PathBase>::next_step(self, step)
    }
}
