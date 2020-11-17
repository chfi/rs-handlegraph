use crate::handle::Handle;

use super::EdgeLists;

use super::index::list;

/// Iterator for stepping through an edge list, returning Handles.
pub struct EdgeListHandleIter<'a> {
    edge_list_iter: list::Iter<'a, EdgeLists>,
}

impl<'a> EdgeListHandleIter<'a> {
    pub(super) fn new(edge_list_iter: list::Iter<'a, EdgeLists>) -> Self {
        Self { edge_list_iter }
    }
}

impl<'a> Iterator for EdgeListHandleIter<'a> {
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let (_, (handle, _)) = self.edge_list_iter.next()?;
        Some(handle)
    }
}
