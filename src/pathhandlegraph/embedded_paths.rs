use crate::handle::Handle;

use super::{PathBase, PathId, PathRef, PathRefMut, StepHandle, StepUpdate};

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
    type Path: PathRef;

    fn path_ref(self, id: PathId) -> Option<Self::Path>;
}

pub trait AllPathRefs: PathRefs {
    type PathIds: AllPathIds;

    fn all_paths_ref(self) -> Vec<Self::Path>;
}

pub trait PathRefsMut: Sized {
    type PathMut: PathRefMut;

    fn path_mut(self, id: PathId) -> Option<Self::PathMut>;
}

pub trait AllPathRefsMut {
    // type MultiCtx: Sized;
    type MutCtx: PathRefMut;
    type PathRefsMut: Iterator<Item = Self::MutCtx>;

    fn all_paths_mut(self) -> Self::PathRefsMut;
}

pub trait WithPathRefsMut: Sized {
    type MutCtx: PathRefMut;

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

/// A collection of embedded paths in a graph.
pub trait EmbeddedPaths: Sized {
    /// Iterator through all path IDs in the graph
    type AllPaths: Iterator<Item = PathId>;

    /// Iterator through the name of a given path
    type PathName: Iterator<Item = u8>;

    /// The concrete underlying path
    type Path: PathRef;

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
    fn create_path(&mut self, name: &[u8], circular: bool) -> PathId;
    fn remove_path(&mut self, id: PathId);
}

pub trait PathOccurrences: EmbeddedPaths {
    type Occurrences: Iterator<Item = StepHandle>;

    /// Iterator through all the steps on the given handle, across all
    /// paths.
    fn steps_on_handle(self, handle: Handle) -> Self::Occurrences;
}

pub trait EmbeddedMutablePath: EmbeddedPaths {
    type PathMut: PathRefMut;

    fn get_path_mut(self, path_id: PathId) -> Option<Self::PathMut>;
}
