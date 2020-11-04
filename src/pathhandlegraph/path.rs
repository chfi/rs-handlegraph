use crate::handle::Handle;

/// A unique identifier for a single path.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PathId(pub u64);

pub trait PathStep: Sized + Copy + Eq {
    fn handle(&self) -> Handle;
}

pub trait PathBase: Sized {
    type Step: PathStep;

    type StepIx: Sized + Copy + Eq;
}

impl<'a, T> PathBase for &'a T
where
    T: PathBase,
{
    type Step = T::Step;

    type StepIx = T::StepIx;
}

impl<'a, T> PathBase for &'a mut T
where
    T: PathBase,
{
    type Step = T::Step;

    type StepIx = T::StepIx;
}

/// Abstraction of an immutable embedded path.
pub trait PathRef: Copy + PathBase {
    /// The iterator that will step through the length of the path.
    type Steps: DoubleEndedIterator<Item = Self::Step>;

    // Return a step iterator, starting from the first step on the path.
    fn steps(self) -> Self::Steps;

    fn len(self) -> usize;

    #[inline]
    fn is_empty(self) -> bool {
        self.len() == 0
    }

    fn circular(self) -> bool;

    fn first_step(self) -> Self::Step;

    fn last_step(self) -> Self::Step;

    fn next_step(self, step: Self::Step) -> Option<Self::Step>;

    fn prev_step(self, step: Self::Step) -> Option<Self::Step>;

    fn contains(self, handle: Handle) -> bool {
        self.steps().any(|s| s.handle() == handle)
    }

    /*
    fn handle_at(self, step: Self::Step) -> Option<Handle>;
    */

    /*
    fn before_step(self) -> StepHandle;

    fn after_step(self) -> StepHandle;
    */

    /*
    fn bases_len(self) -> usize;

    fn step_at_base(self, pos: usize) -> Option<StepHandle>;
    */
}

pub trait PathRefMutSteps: PathBase {
    // type StepUpdate: Sized + Copy;

    fn append_step(&mut self, handle: Handle) -> (Handle, Self::StepIx);

    fn prepend_step(&mut self, handle: Handle) -> (Handle, Self::StepIx);

    /*
    fn append_iter<I>(self, iter: I) -> Vec<StepUpdate>
    where
        I: IntoIterator<Item = Handle>;

    fn prepend_iter<I>(self, iter: I) -> Vec<StepUpdate>
    where
        I: IntoIterator<Item = Handle>;
    */

    // fn rewrite_segment<I>(self, iter: I, from: Self::Step, to: Self::Step) -> Vec<StepUpdate>;
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
