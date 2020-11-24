/*!

`HashGraph` node definition and occurrence iterator

*/

use fnv::FnvHashMap;

use crate::handle::Handle;
use crate::pathhandlegraph::PathId;

use super::path::StepIx;

#[derive(Debug, Clone)]
pub struct Node {
    pub sequence: Vec<u8>,
    pub left_edges: Vec<Handle>,
    pub right_edges: Vec<Handle>,
    pub occurrences: FnvHashMap<PathId, usize>,
}

impl Node {
    pub fn new(sequence: &[u8]) -> Node {
        Node {
            sequence: sequence.into(),
            left_edges: vec![],
            right_edges: vec![],
            occurrences: FnvHashMap::default(),
        }
    }
}

/// Iterator on the path occurrences of a node
pub struct OccurIter<'a> {
    pub(super) iter: std::collections::hash_map::Iter<'a, PathId, usize>,
}

impl<'a> Iterator for OccurIter<'a> {
    type Item = (PathId, StepIx);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (id, offset) = self.iter.next()?;
        Some((*id, StepIx::Step(*offset)))
    }
}
