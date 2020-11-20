use crate::handle::Handle;

use super::{
    MutPath, PathBase, PathId, PathStep, PathSteps, StepHandle, StepUpdate,
};

pub trait GraphPaths: Sized {
    type PathName: Iterator<Item = u8>;

    type Path: PathBase<Step = Self::Step, StepIx = Self::StepIx>;

    // "Aliases" to the associated types on `Path`, to avoid having to
    // deal with fully qualified syntax everywhere in the subtrait
    // definitions that use Step and StepIx
    // See https://github.com/rust-lang/rust/issues/38078#issuecomment-386796174
    type Step: PathStep;

    type StepIx: Sized + Copy + Eq;

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

    fn path_count(&self) -> usize;

    fn path_len(&self, id: PathId) -> Option<usize>;

    fn path_circular(&self) -> Option<bool>;

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

pub trait GraphMutablePaths: GraphPaths {
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
    ) -> Option<Vec<Self::StepIx>>;

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
    fn get_path_ref<'a>(&'a self, id: PathId) -> Option<&'a Self::Path>;

    fn get_path_mut_ref<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<&'a mut Self::Path>;
}

pub trait GraphPathsRefAlt: GraphPaths {
    type PathRef: MutPath<Step = Self::Step, StepIx = Self::StepIx>;

    fn get_path_ref_<'a>(&'a self, id: PathId) -> Option<&'a Self::PathRef>;

    fn get_path_mut_ref_<'a>(
        &'a mut self,
        id: PathId,
    ) -> Option<&'a mut Self::PathRef>;
}

pub trait EmbeddedPaths_: Sized {
    type NameIter: Iterator<Item = u8>;

    // type Path: PathBase;

    fn get_path_id(&self, name: &[u8]) -> Option<PathId>;

    fn get_path_name(&self, id: PathId) -> Option<Self::NameIter>;

    #[inline]
    fn get_path_name_vec(&self, id: PathId) -> Option<Vec<u8>> {
        self.get_path_name(id).map(|name| name.collect())
    }

    // fn get_path_ref(&self, id: PathId) -> Option<Self::Path>;

    #[inline]
    fn has_path(&self, name: &[u8]) -> bool {
        self.get_path_id(name).is_some()
    }

    fn path_count(&self) -> usize;
}

pub trait EmbeddedPathRef {
    type Path: PathBase;

    fn get_path_ref(&self, id: PathId) -> Option<Self::Path>;
}

impl<'a, T> EmbeddedPaths_ for &'a T
where
    T: EmbeddedPaths_,
{
    type NameIter = T::NameIter;

    // type Path = T::Path;

    fn get_path_id(&self, name: &[u8]) -> Option<PathId> {
        T::get_path_id(self, name)
    }

    fn get_path_name(&self, id: PathId) -> Option<Self::NameIter> {
        T::get_path_name(self, id)
    }

    // fn get_path_ref(&self, id: PathId) -> Option<Self::Path> {
    // T::get_path_ref(self, id)
    // }

    fn path_count(&self) -> usize {
        T::path_count(self)
    }
}

/*
impl<'a, T> EmbeddedPaths_ for &'a mut T
where
    T: EmbeddedPaths_,
{
    type NameIter = T::NameIter;

    type Path = T::Path;

    fn get_path_id(&self, name: &[u8]) -> Option<PathId> {
        T::get_path_id(self, name)
    }

    fn get_path_name(&self, id: PathId) -> Option<Self::NameIter> {
        T::get_path_name(self, id)
    }

    fn get_path_ref(&self, id: PathId) -> Option<Self::Path> {
        T::get_path_ref(self, id)
    }

    fn path_count(&self) -> usize {
        T::path_count(self)
    }
}
*/

pub trait IntoPaths: EmbeddedPathRef {
    type PathIdIter: Iterator<Item = PathId>;

    type PathIter: Iterator<Item = Self::Path>;

    fn all_path_ids(self) -> Self::PathIdIter;

    fn all_paths(self) -> Self::PathIter;
}

pub trait MutEmbeddedPaths_: EmbeddedPaths_ {
    type PathMut: MutPath;

    fn get_path_mut(&mut self, id: PathId) -> Option<&mut Self::PathMut>;

    fn create_path(&mut self, name: &[u8]) -> Option<PathId>;

    fn destroy_path(&mut self, id: PathId) -> bool;
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

pub trait PathRefs: Sized {
    type Path: PathSteps;

    fn path_ref(self, id: PathId) -> Option<Self::Path>;
}

pub trait AllPathRefs: PathRefs {
    type PathIds: AllPathIds;

    fn all_paths_ref(self) -> Vec<Self::Path>;
}

pub trait PathRefsMut: Sized {
    type PathMut: MutPath;

    fn path_mut(self, id: PathId) -> Option<Self::PathMut>;
}

pub trait AllPathRefsMut {
    // type MultiCtx: Sized;
    type MutCtx: MutPath;
    type PathRefsMut: Iterator<Item = Self::MutCtx>;

    fn all_paths_mut(self) -> Self::PathRefsMut;
}

pub trait WithPathRefsMut: Sized {
    type MutCtx: MutPath;

    #[allow(clippy::type_complexity)]
    fn with_path_mut<F>(
        self,
        id: PathId,
        f: F,
    ) -> Option<Vec<StepUpdate<<Self::MutCtx as PathBase>::StepIx>>>
    where
        for<'b> F: Fn(
            &mut Self::MutCtx,
        )
            -> Vec<StepUpdate<<Self::MutCtx as PathBase>::StepIx>>;

    #[allow(clippy::type_complexity)]
    fn with_paths_mut<F>(
        self,
        f: F,
    ) -> Vec<(PathId, Vec<StepUpdate<<Self::MutCtx as PathBase>::StepIx>>)>
    where
        for<'b> F: Fn(
            PathId,
            &mut Self::MutCtx,
        )
            -> Vec<StepUpdate<<Self::MutCtx as PathBase>::StepIx>>;
}

/*
/// A collection of embedded paths in a graph.
pub trait EmbeddedPaths: Sized {
    /// Iterator through all path IDs in the graph
    type AllPaths: Iterator<Item = PathId>;

    /// Iterator through the name of a given path
    type PathName: Iterator<Item = u8>;

    /// The concrete underlying path
    type Path: PathSteps;

    /// Iterator through all the path IDs in this graph
    fn all_path_ids(self) -> Self::AllPaths;

    /// Retrieve a path by name.
    fn get_path(self, path_id: PathId) -> Option<Self::Path>;

    /// Get the path ID for the given name, if it exists
    fn lookup_path_id(self, name: &[u8]) -> Option<PathId>;

    /// Get the text name for the given ID, if it exists
    fn get_path_name(self, path_id: PathId) -> Option<Self::PathName>;

    // fn contains_path(self, name: &[u8]) -> bool;

    /// The number of embedded paths
    fn path_count(self) -> usize;
}

pub trait MutEmbeddedPaths {
    type StepIx: Sized + Copy + Eq;

    fn create_path(&mut self, name: &[u8], circular: bool) -> PathId;
    fn remove_path(&mut self, id: PathId);

    fn append_step_on(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    fn prepend_step_on(
        &mut self,
        id: PathId,
        handle: Handle,
    ) -> Option<Self::StepIx>;

    fn rewrite_segment_on(
        &mut self,
        id: PathId,
        begin: Self::StepIx,
        end: Self::StepIx,
    ) -> Option<(Self::StepIx, Self::StepIx)>;
}

*/

/*
pub trait PathOccurrences: EmbeddedPaths {
    type Occurrences: Iterator<Item = StepHandle>;

    /// Iterator through all the steps on the given handle, across all
    /// paths.
    fn steps_on_handle(self, handle: Handle) -> Self::Occurrences;
}

pub trait EmbeddedMutablePath: EmbeddedPaths {
    type PathMut: MutPath;

    fn get_path_mut(self, path_id: PathId) -> Option<Self::PathMut>;
}

*/
