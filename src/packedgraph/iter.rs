#![allow(dead_code)]
#![allow(unused_assignments)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_imports)]

use bio::alphabets::dna;
use bstr::{BString, ByteSlice};

use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::MutableHandleGraph,
};

use super::edges::{
    EdgeListIter, EdgeListIx, EdgeLists, EdgeRecord, EdgeVecIx,
};
use super::graph::{PackedGraph, PackedSeqIter, SeqRecordIx, Sequences};
use super::nodes::{GraphRecordIx, GraphVecIx, NodeIdIndexMap, NodeRecords};

/// Iterator over a PackedGraph's handles. For every non-zero value in
/// the PackedDeque holding the PackedGraph's node ID mappings, the
/// corresponding index is mapped back to the original ID and yielded
/// by the iterator.
pub struct PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    iter: std::iter::Enumerate<I>,
    min_id: usize,
}

impl<I> PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    pub(super) fn new(iter: I, min_id: usize) -> Self {
        let iter = iter.enumerate();
        Self { iter, min_id }
    }
}

impl<I> Iterator for PackedHandlesIter<I>
where
    I: Iterator<Item = u64>,
{
    type Item = Handle;

    #[inline]
    fn next(&mut self) -> Option<Handle> {
        while let Some((ix, id)) = self.iter.next() {
            if id != 0 {
                let n_id = ix + self.min_id;
                return Some(Handle::pack(n_id, false));
            }
        }
        None
    }
}

/// Iterator for stepping through an edge list, returning Handles.
pub struct EdgeListHandleIter<'a> {
    edge_list_iter: EdgeListIter<'a>,
}

impl<'a> EdgeListHandleIter<'a> {
    pub(super) fn new(edge_list_iter: EdgeListIter<'a>) -> Self {
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
