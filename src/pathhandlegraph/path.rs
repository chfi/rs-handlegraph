use crate::handle::{Direction, Edge, Handle, NodeId};

/// A unique identifier for a single path.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PathId(pub u64);

/// A step along a path; the path context is implicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathStep {
    Before,
    After,
    Step(usize),
}

impl PathStep {
    #[inline]
    pub fn before(&self) -> bool {
        *self == PathStep::Before
    }

    #[inline]
    pub fn after(&self) -> bool {
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

pub trait PathBase: Sized {
    type Step: Copy + Eq;
}

impl<'a, T> PathBase for &'a T
where
    T: PathBase,
{
    type Step = T::Step;
}

impl<'a, T> PathBase for &'a mut T
where
    T: PathBase,
{
    type Step = T::Step;
}

/// Abstraction of an immutable embedded path.
pub trait PathRef: Copy + PathBase {
    /// The iterator that will step through the length of the path.
    type Steps: Iterator<Item = Self::Step>;

    /// Return a step iterator, starting from the first step on the path.
    fn steps(self) -> Self::Steps;

    fn len(self) -> usize;

    fn circular(self) -> bool;

    // fn first_step(self) -> StepHandle;

    // fn last_step(self) -> StepHandle;

    fn handle_at(self, step: Self::Step) -> Option<Handle>;

    fn contains(self, handle: Handle) -> bool;

    // fn next_step(self, step: PathStep) -> Option<PathStep>;

    // fn prev_step(self, step: PathStep) -> Option<PathStep>;

    /*
    fn before_step(self) -> StepHandle;

    fn after_step(self) -> StepHandle;
    */

    /*
    fn bases_len(self) -> usize;

    fn step_at_base(self, pos: usize) -> Option<StepHandle>;
    */
}

/// An embedded path that can also be mutated by appending or
/// prepending steps, or rewriting parts of it.
pub trait PathRefMut: PathBase {
    fn append(self, handle: Handle) -> Self::Step;

    fn prepend(self, handle: Handle) -> Self::Step;

    // fn rewrite_segment(
    //     self,
    //     from: PathStep,
    //     to: PathStep,
    //     new_segment: &[Handle],
    // ) -> Option<(PathStep, PathStep)>;

    fn set_circularity(self, circular: bool);
}