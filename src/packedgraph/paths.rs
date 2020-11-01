use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{AdditiveHandleGraph, MutableHandleGraph},
};

use std::num::NonZeroUsize;

use super::GraphRecordIx;

use crate::pathhandlegraph::*;

use crate::packed::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedPathStep(Option<NonZeroUsize>);

impl PackedPathStep {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    #[inline]
    fn from_zero_based(x: usize) -> Self {
        let x = x + 1;
        Self::new(x)
    }

    #[inline]
    fn to_zero_based(self) -> Option<usize> {
        if let Some(ix) = self.0 {
            Some(ix.get() - 1)
        } else {
            None
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn empty() -> Self {
        Self(None)
    }
    #[inline]
    pub(super) fn is_null(&self) -> bool {
        self.0.is_none()
    }

    #[inline]
    pub(super) fn as_vec_value(&self) -> u64 {
        match self.0 {
            None => 0,
            Some(v) => v.get() as u64,
        }
    }

    #[inline]
    pub(super) fn from_vec_value(x: u64) -> Self {
        Self(NonZeroUsize::new(x as usize))
    }
}

pub struct PackedPath {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
    path_id: PathId,
    head: PackedPathStep,
    tail: PackedPathStep,
}

impl PackedPath {
    pub(super) fn new(path_id: PathId) -> Self {
        Self {
            path_id,
            steps: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            head: PackedPathStep::empty(),
            tail: PackedPathStep::empty(),
        }
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub(super) fn append_step(&mut self, handle: Handle) -> PackedPathStep {
        let ix = PackedPathStep::from_zero_based(self.len());
        self.steps.append(handle.as_integer());

        if self.head.is_null() {
            self.head = PackedPathStep::from_zero_based(0);
            self.tail = self.head;
        }

        self.links.append(ix as u64);
        self.links.append(0);

        if !self.tail.is_null() {
            // this is definitely super wrong
            self.links
                .set(ix - 1, self.tail.to_zero_based().unwrap() as u64);
        }

        ix
    }

    pub(super) fn prepend_step(&mut self, handle: Handle) -> PackedPathStep {
        let ix = PackedPathStep::from_zero_based(self.len());
        self.steps.append(handle.as_integer());

        if self.head.is_null() {
            self.head = PackedPathStep::from_zero_based(0);
            self.tail = self.head;
        }

        self.links.append(0);
        self.links.append(self.head.to_zero_based().unwrap() as u64);

        // self.links.set(self.hea

        // if !self.tail.is_null() {
        // this is definitely super wrong
        self.links
            .set(ix - 1, self.tail.to_zero_based().unwrap() as u64);
        // }

        ix
    }
}

pub struct PackedPathSteps<'a> {
    path: &'a PackedPath,
    current_step: usize,
    finished: bool,
}

impl<'a> PackedPathSteps<'a> {
    fn new(path: &'a PackedPath) -> Self {
        Self {
            path,
            current_step: 0,
            finished: false,
        }
    }

    /*
    fn next(&mut self) -> Option<(usize, Handle)> {
        if self.finished {
            return None;
        }

        let handle = Handle::from_integer(self.steps.get(self.current_step));
        let index = self.current_step;

        let link = self.current_step += 1;
    }
    */
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeOccurIx(Option<NonZeroUsize>);

impl NodeOccurIx {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }

    #[inline]
    fn from_zero_based<I: Into<usize>>(x: I) -> Self {
        let x = x.into() + 1;
        Self::new(x)
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn empty() -> Self {
        Self(None)
    }

    #[inline]
    pub(super) fn is_null(&self) -> bool {
        self.0.is_none()
    }

    #[inline]
    pub(super) fn as_vec_value(&self) -> u64 {
        match self.0 {
            None => 0,
            Some(v) => v.get() as u64,
        }
    }

    #[inline]
    pub(super) fn from_vec_value(x: u64) -> Self {
        Self(NonZeroUsize::new(x as usize))
    }

    #[inline]
    fn from_graph_record_ix(g_ix: GraphRecordIx) -> Self {
        if g_ix.is_null() {
            Self::empty()
        } else {
            let x = g_ix.as_vec_value() as usize;
            Self::new(x)
        }
    }

    #[inline]
    pub(super) fn as_vec_ix(&self) -> Option<usize> {
        let x = self.0?.get();
        Some(x - 1)
    }
}

pub struct OccurRecord {
    path_id: PathId,
    offset: usize,
    next: NodeOccurRecordIx,
}

pub struct NodeOccurrences {
    path_ids: PagedIntVec,
    node_occur_offsets: PagedIntVec,
    node_occur_next: PagedIntVec,
}

impl Default for NodeOccurrences {
    fn default() -> Self {
        Self {
            path_ids: PagedIntVec::new(super::graph::WIDE_PAGE_WIDTH),
            node_occur_offsets: PagedIntVec::new(
                super::graph::NARROW_PAGE_WIDTH,
            ),
            node_occur_next: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }
}

pub type NodeOccurRecordIx = usize;

impl NodeOccurrences {
    pub(super) fn append_record(
        &mut self,
        g_ix: GraphRecordIx,
    ) -> Option<NodeOccurRecordIx> {
        let node_rec_ix = self.path_ids.len();

        self.path_ids.append(0);
        self.node_occur_offsets.append(0);
        self.node_occur_next.append(0);

        Some(node_rec_ix)
    }

    pub(super) fn set_record(
        &mut self,
        ix: NodeOccurRecordIx,
        path_id: PathId,
        offset: usize,
        next: NodeOccurRecordIx,
    ) -> bool {
        if ix >= self.path_ids.len() {
            return false;
        }

        self.path_ids.set(ix, path_id.0);
        self.node_occur_offsets.set(offset as u64);
        self.node_occur_next.set(next as u64);

        true
    }

    pub(super) fn get_record(
        &self,
        ix: NodeOccurRecordIx,
    ) -> Option<OccurRecord> {
        if ix >= self.path_ids.len() {
            return None;
        }

        let path_id = PathId(self.path_ids.get(ix));
        let offset = self.node_occur_offsets.get(ix) as usize;
        let next = self.node_occur_next.get(ix) as usize;

        Some(OccurRecord {
            path_id,
            offset,
            next,
        })
    }

    pub(super) fn iter(
        &self,
        ix: NodeOccurRecordIx,
    ) -> NodeOccurrencesIter<'_> {
        NodeOccurrencesIter::new(self, ix)
    }

    pub(super) fn set_last_next(
        &mut self,
        ix: NodeOccurRecordIx,
        next: NodeOccurRecordIx,
    ) {
        let mut cur_ix = ix;
        for record in self.iter(ix) {
            if record.next != 0 {
                cur_ix = record.next;
            }
        }

        self.node_occur_next.set(cur_ix, next as u64);
    }
}

pub struct NodeOccurrencesIter<'a> {
    occurrences: &'a NodeOccurrences,
    current_rec_ix: NodeOccurRecordIx,
}

impl<'a> NodeOccurrencesIter<'a> {
    fn new(
        occurrences: &'a NodeOccurrences,
        current_rec_ix: NodeOccurRecordIx,
    ) -> Self {
        Self {
            occurrences,
            current_rec_ix,
        }
    }
}

impl<'a> Iterator for NodeOccurrencesIter<'a> {
    type Item = OccurRecord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_rec_ix == 0 {
            None
        } else {
            // TODO I shouldn't use unwrap() here; there are better
            // ways of making sure this is consistent
            let item =
                self.occurrences.get_record(self.current_rec_ix).unwrap();
            self.current_rec_ix = item.next;
            Some(item)
        }
    }
}
