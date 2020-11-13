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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepUpdate<StepIx: Sized + Copy + Eq> {
    Insert { handle: Handle, step: StepIx },
    Remove { handle: Handle, step: StepIx },
    // InsertSegment {
    //     start: StepIx,
    //     steps: Vec<(Handle, StepIx)>,
    // },
}

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
pub trait PathRef: PathBase {
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

/// An embedded path that can also be mutated by appending or
/// prepending steps, or rewriting parts of it.
pub trait PathRefMut: PathBase {
    fn append_step(&mut self, handle: Handle) -> StepUpdate<Self::StepIx>;

    fn prepend_step(&mut self, handle: Handle) -> StepUpdate<Self::StepIx>;

    fn insert_step_after(
        &mut self,
        ix: Self::StepIx,
        handle: Handle,
    ) -> StepUpdate<Self::StepIx>;

    fn remove_step(
        &mut self,
        step: Self::StepIx,
    ) -> Option<StepUpdate<Self::StepIx>>;

    // fn rewrite_segment(
    //     self,
    //     from: PathStep,
    //     to: PathStep,
    //     new_segment: &[Handle],
    // ) -> Option<(PathStep, PathStep)>;

    fn set_circularity(&mut self, circular: bool);
}
