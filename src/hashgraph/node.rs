use bstr::BString;
use std::collections::HashMap;

use crate::handle::Handle;

use super::PathId;

#[derive(Debug, Clone)]
pub struct Node {
    pub sequence: BString,
    pub left_edges: Vec<Handle>,
    pub right_edges: Vec<Handle>,
    pub occurrences: HashMap<PathId, usize>,
}

impl Node {
    pub fn new(sequence: &[u8]) -> Node {
        Node {
            sequence: sequence.into(),
            left_edges: vec![],
            right_edges: vec![],
            occurrences: HashMap::new(),
        }
    }
}
