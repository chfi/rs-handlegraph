use crate::handle::Handle;

use super::{PathId, PathRef, PathRefMut, StepHandle};

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

pub trait AdditivePaths: EmbeddedPaths {
    fn create_path(self, name: &[u8], circular: bool) -> PathId;

    // fn destroy_path(self, path_id: PathId) -> bool;
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
