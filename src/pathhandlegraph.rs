use crate::handle::{Direction, Edge, Handle, NodeId};

mod embedded_paths;
mod path;

pub use self::embedded_paths::*;
pub use self::path::{PathId, PathRef, PathRefMut, PathStep, StepHandle};
