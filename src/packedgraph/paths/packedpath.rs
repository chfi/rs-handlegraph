#![allow(dead_code)]

use crate::handle::Handle;

use std::num::NonZeroUsize;

use super::super::graph::NARROW_PAGE_WIDTH;

use super::{
    OneBasedIndex, PackedDoubleList, PackedList, PackedListIter, RecordIndex,
};

use super::super::NodeIdIndexMap;

use crate::pathhandlegraph::PathId;

use super::properties::*;

use super::occurrences::{NodeOccurRecordIx, NodeOccurrences};

use crate::packed::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathStepIx(Option<NonZeroUsize>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PathLinkRecordIx(usize);

crate::impl_one_based_index!(PathStepIx);

impl RecordIndex for PathLinkRecordIx {
    const RECORD_WIDTH: usize = 2;

    #[inline]
    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self> {
        ix.to_record_start(Self::RECORD_WIDTH).map(PathLinkRecordIx)
    }

    #[inline]
    fn to_one_based_ix<I: OneBasedIndex>(self) -> I {
        I::from_record_start(self.0, Self::RECORD_WIDTH)
    }

    #[inline]
    fn record_ix(self, offset: usize) -> usize {
        self.0 + offset
    }
}

/// A reified record of a step in a PackedPath, with `handle` taken
/// from the `steps` list, and `prev` & `next` taking from the `links`
/// list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedStep {
    pub(super) handle: Handle,
    pub(super) prev: PathStepIx,
    pub(super) next: PathStepIx,
}

pub struct PackedPath {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
}

impl PackedPath {
    pub(super) fn new() -> Self {
        Self {
            steps: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
        }
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub(super) fn append_handle(&mut self, handle: Handle) -> PathStepIx {
        let new_ix = PathStepIx::from_zero_based(self.len());
        self.steps.append(handle.pack());
        self.links.append(0);
        self.links.append(0);
        new_ix
    }

    fn link_record(&self, ix: PathStepIx) -> Option<(PathStepIx, PathStepIx)> {
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;
        let prev = self.links.get_unpack(link_ix.record_ix(0));
        let next = self.links.get_unpack(link_ix.record_ix(1));
        Some((prev, next))
    }

    fn step_record(&self, ix: PathStepIx) -> Option<Handle> {
        let step_ix = ix.to_record_start(1)?;
        let step = self.steps.get_unpack(step_ix);
        Some(step)
    }

    fn set_link(&mut self, from: PathStepIx, to: PathStepIx) -> Option<()> {
        let from_next_ix = from.to_record_ix(2, 1)?;
        let to_prev_ix = to.to_record_ix(2, 0)?;

        self.links.set_pack(from_next_ix, to);
        self.links.set_pack(to_prev_ix, from);

        Some(())
    }

    pub(super) fn prev_step(&self, ix: PathStepIx) -> Option<PathStepIx> {
        let link_ix = ix.to_record_ix(2, 0)?;
        let link = self.links.get_unpack(link_ix);
        Some(link)
    }

    pub(super) fn next_step(&self, ix: PathStepIx) -> Option<PathStepIx> {
        let link_ix = ix.to_record_ix(2, 1)?;
        let link = self.links.get_unpack(link_ix);
        Some(link)
    }

    pub(super) fn insert_after(
        &mut self,
        ix: PathStepIx,
        handle: Handle,
    ) -> Option<PathStepIx> {
        let new_ix = PathStepIx::from_zero_based(self.len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.as_integer());

        let ix_next: PathStepIx = self.links.get_unpack(link_ix.record_ix(1));

        if let Some(next_link_ix) = PathLinkRecordIx::from_one_based_ix(ix_next)
        {
            self.links
                .set_pack(next_link_ix.record_ix(0), new_ix.pack());
        }

        self.links.append(ix.pack());
        self.links.append(ix_next.pack());

        self.links.set(link_ix.record_ix(1), new_ix.pack());

        Some(new_ix)
    }

    pub(super) fn insert_before(
        &mut self,
        ix: PathStepIx,
        handle: Handle,
    ) -> Option<PathStepIx> {
        let new_ix = PathStepIx::from_zero_based(self.len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.pack());

        let ix_prev: PathStepIx = self.links.get_unpack(link_ix.record_ix(0));

        if let Some(prev_link_ix) = PathLinkRecordIx::from_one_based_ix(ix_prev)
        {
            self.links
                .set_pack(prev_link_ix.record_ix(1), new_ix.pack());
        }

        self.links.append(ix_prev.pack());
        self.links.append(ix.pack());

        self.links.set_pack(link_ix.record_ix(0), new_ix);

        Some(new_ix)
    }

    pub fn iter(
        &self,
        head: PathStepIx,
        tail: PathStepIx,
    ) -> PackedListIter<'_, PackedPath> {
        PackedListIter::new_double(self, head, tail)
    }
}

impl PackedList for PackedPath {
    type ListPtr = PathStepIx;
    type ListRecord = PackedStep;

    #[inline]
    fn next_pointer(rec: &PackedStep) -> PathStepIx {
        rec.next
    }

    #[inline]
    fn get_record(&self, ptr: PathStepIx) -> Option<PackedStep> {
        let link_ix = PathLinkRecordIx::from_one_based_ix(ptr)?;
        let prev = self.links.get_unpack(link_ix.record_ix(0));
        let next = self.links.get_unpack(link_ix.record_ix(1));

        let step_ix = ptr.to_record_start(1)?;
        let handle = self.steps.get_unpack(step_ix);

        Some(PackedStep { prev, next, handle })
    }
}

impl PackedDoubleList for PackedPath {
    #[inline]
    fn prev_pointer(rec: &PackedStep) -> PathStepIx {
        rec.prev
    }
}

pub struct PackedPathRef<'a> {
    pub path_id: PathId,
    pub path: &'a PackedPath,
    pub properties: PathPropertyRef<'a>,
}

pub struct PackedPathRefMut<'a> {
    pub path_id: PathId,
    pub path: &'a mut PackedPath,
    pub properties: PathPropertyMut<'a>,
}

impl<'a> PackedPathRefMut<'a> {
    pub(super) fn new(
        path_id: PathId,
        path: &'a mut PackedPath,
        properties: PathPropertyMut<'a>,
    ) -> Self {
        PackedPathRefMut {
            path_id,
            path,
            properties,
        }
    }

    #[must_use]
    pub(super) fn append_handle(
        &mut self,
        handle: Handle,
    ) -> (Handle, PathStepIx) {
        let tail = self.properties.get_tail();
        let step = self.path.append_handle(handle);

        // add back link from new step to old tail

        let new_prev_ix = step.to_record_ix(2, 0).unwrap();
        self.path.links.set_pack(new_prev_ix, tail);

        // just in case the path was empty, set the head as well
        if self.properties.get_head().is_null() {
            self.properties.set_head(step);
        }

        if let Some(tail_next_ix) = tail.to_record_ix(2, 1) {
            // add forward link from old tail to new step
            self.path.links.set_pack(tail_next_ix, step);
        }
        // set the new tail
        self.properties.set_tail(step);

        (handle, step)
    }

    #[must_use]
    pub(super) fn prepend_handle(
        &mut self,
        handle: Handle,
    ) -> (Handle, PathStepIx) {
        let head = self.properties.get_head();
        let step = self.path.append_handle(handle);

        // add forward link from new step to old head
        let new_next_ix = step.to_record_ix(2, 1).unwrap();
        self.path.links.set_pack(new_next_ix, head);

        // just in case the path was empty, set the tail as well
        if self.properties.get_tail().is_null() {
            self.properties.set_tail(step);
        }

        if let Some(head_prev_ix) = head.to_record_ix(2, 01) {
            // add back link from old head to new step
            self.path.links.set_pack(head_prev_ix, step);
        }
        // set the new head
        self.properties.set_head(step);

        (handle, step)

        // self.occurrences
        //     .prepend_occurrence(node_occur_ix, self.path_id, step);
        // let node_occur_ix = self.occurrences.append_record();

        // self.occurrences.set_record(
        //     node_occur_ix,
        //     self.path_id,
        //     step,
        //     NodeOccurRecordIx::null(),
        // );
    }
}
