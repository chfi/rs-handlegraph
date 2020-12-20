use crate::{
    handle::{Edge, Handle, NodeId},
    handlegraph::HandleGraphRef,
    mutablehandlegraph::*,
    pathhandlegraph::*,
};

use crate::packedgraph::paths::StepPtr;
use crate::packedgraph::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinkPath {
    pub from_cons_name: Vec<u8>,
    pub to_cons_name: Vec<u8>,
    pub from_cons_path: PathId,
    pub to_cons_path: PathId,
    length: usize,
    hash: u64,
    begin: StepPtr,
    end: StepPtr,
    path: PathId,
    is_reverse: bool,
    jump_len: usize,
    rank: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinkRange {
    start: NodeId,
    end: NodeId,
    path: PathId,
}
