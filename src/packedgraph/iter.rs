use crate::handle::Handle;

use super::EdgeListIx;
use super::{index::list, EdgeLists};

use fnv::FnvHashSet;

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

pub struct EdgeListHandleIterTrace<'a> {
    edge_list_iter: list::Iter<'a, EdgeLists>,
    flip: bool,
    visited_indices: FnvHashSet<EdgeListIx>,
}

impl<'a> EdgeListHandleIterTrace<'a> {
    pub(super) fn new(
        edge_list_iter: list::Iter<'a, EdgeLists>,
        flip: bool,
    ) -> Self {
        Self {
            edge_list_iter,
            flip,
            visited_indices: Default::default(),
        }
    }

    pub(super) fn new_continue(
        edge_list_iter: list::Iter<'a, EdgeLists>,
        flip: bool,
        visited_indices: FnvHashSet<EdgeListIx>,
    ) -> Self {
        Self {
            edge_list_iter,
            flip,
            visited_indices,
        }
    }

    pub fn visit_now(
        edge_list_iter: list::Iter<'a, EdgeLists>,
        flip: bool,
        visited_in: &mut FnvHashSet<EdgeListIx>,
    ) {
        let visited = std::mem::take(visited_in);
        let mut iter_trace = Self::new_continue(edge_list_iter, flip, visited);
        iter_trace.find_all();
        std::mem::swap(visited_in, &mut iter_trace.visited_indices);
    }

    pub fn visited(&self) -> &FnvHashSet<EdgeListIx> {
        &self.visited_indices
    }

    pub fn find_all(&mut self) -> &FnvHashSet<EdgeListIx> {
        while let Some(_) = self.next() {}
        self.visited()
    }

    pub fn into_visited(self) -> FnvHashSet<EdgeListIx> {
        self.visited_indices
    }
}

impl<'a> Iterator for EdgeListHandleIterTrace<'a> {
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        let (ix, (handle, _)) = self.edge_list_iter.next()?;
        self.visited_indices.insert(ix);
        if self.flip {
            Some(handle.flip())
        } else {
            Some(handle)
        }
    }
}
