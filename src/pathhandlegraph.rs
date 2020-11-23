/*!

Traits and utilities for accessing and manipulating paths embedded in
a graph.

The interfaces for working with the paths of an entire graph are
defined in [`embedded_paths`], while [`path`] deals with single paths,
and references to paths.

*/

pub mod embedded_paths;
pub mod path;

pub mod occurrences;

pub use self::embedded_paths::*;
pub use self::occurrences::*;
pub use self::path::*;
