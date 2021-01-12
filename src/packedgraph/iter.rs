use crate::handle::Handle;

use super::{index::list, EdgeLists};

/// Iterator for stepping through an edge list, returning Handles.
pub struct EdgeListHandleIter<'a> {
    edge_list_iter: list::Iter<'a, EdgeLists>,
    flip: bool,
}

impl<'a> EdgeListHandleIter<'a> {
    pub(super) fn new(
        edge_list_iter: list::Iter<'a, EdgeLists>,
        flip: bool,
    ) -> Self {
        Self {
            edge_list_iter,
            flip,
        }
    }
}

impl<'a> Iterator for EdgeListHandleIter<'a> {
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let (_, (handle, _)) = self.edge_list_iter.next()?;
        if self.flip {
            Some(handle.flip())
        } else {
            Some(handle)
        }
    }
}
