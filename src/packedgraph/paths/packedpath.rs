#![allow(dead_code)]

use crate::handle::Handle;

use std::num::NonZeroUsize;

use super::super::graph::NARROW_PAGE_WIDTH;

use super::{OneBasedIndex, RecordIndex};

use super::super::NodeIdIndexMap;

use crate::packedgraph::index::list;
use list::{PackedDoubleList, PackedList, PackedListMut};

use crate::pathhandlegraph::{PathBase, PathId, PathRef, PathRefMut, PathStep};

use super::properties::*;

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
    pub(crate) handle: Handle,
    pub(crate) prev: PathStepIx,
    pub(crate) next: PathStepIx,
}

#[derive(Debug, Clone)]
pub struct PackedPath {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
    removed_steps: Vec<PathStepIx>,
}

impl PackedPath {
    pub(super) fn new() -> Self {
        Self {
            steps: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            removed_steps: Default::default(),
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

    fn get_step(&self, ix: PathStepIx) -> PackedStep {
        let handle = self.step_record(ix).unwrap();
        let (prev, next) = self.link_record(ix).unwrap();
        PackedStep { handle, prev, next }
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
    ) -> list::Iter<'_, PackedPath> {
        list::Iter::new_double(self, head, tail)
    }

    pub(crate) fn iter_mut(
        &mut self,
        head: PathStepIx,
        tail: PathStepIx,
    ) -> list::IterMut<'_, PackedPath> {
        list::IterMut::new_double(self, head, tail)
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

impl PackedListMut for PackedPath {
    type ListLink = (PathStepIx, PathStepIx);

    #[inline]
    fn get_record_link(record: &PackedStep) -> Self::ListLink {
        (record.prev, record.next)
    }

    #[inline]
    fn link_next(link: Self::ListLink) -> PathStepIx {
        link.1
    }

    #[inline]
    fn remove_at_pointer(&mut self, ptr: PathStepIx) -> Option<Self::ListLink> {
        let step_ix = ptr.to_record_ix(1, 0)?;

        let prev_ix = ptr.to_record_ix(2, 0)?;
        let next_ix = prev_ix + 1;

        let prev_ptr: PathStepIx = self.links.get_unpack(prev_ix);
        let next_ptr: PathStepIx = self.links.get_unpack(next_ix);

        match (prev_ptr.to_record_ix(2, 1), next_ptr.to_record_ix(2, 0)) {
            (Some(p_ix), Some(n_ix)) => {
                // update both pointers
                self.links.set_pack(p_ix, next_ptr);
                self.links.set_pack(n_ix, prev_ptr);
            }
            (None, Some(n_ix)) => {
                // set next's previous pointer to zero
                self.links.set_pack(n_ix, PathStepIx::null());
            }
            (Some(p_ix), None) => {
                // set prev's next pointer to zero
                self.links.set_pack(p_ix, PathStepIx::null());
            }
            (None, None) => (),
        }

        self.steps.set(step_ix, 0);

        self.links.set(prev_ix, 0);
        self.links.set(next_ix, 0);

        self.removed_steps.push(ptr);

        Some((prev_ptr, next_ptr))
    }

    #[inline]
    fn remove_next(&mut self, ptr: PathStepIx) -> Option<()> {
        let (_prev, next) = self.link_record(ptr)?;
        let _ = self.remove_at_pointer(next)?;

        Some(())
    }
}

impl PackedDoubleList for PackedPath {
    #[inline]
    fn prev_pointer(rec: &PackedStep) -> PathStepIx {
        rec.prev
    }
}

#[derive(Clone, Copy)]
pub struct PackedPathRef<'a> {
    pub(super) path_id: PathId,
    pub(super) path: &'a PackedPath,
    // pub(super) properties: PathPropertyRef<'a>,
    pub(super) properties: PathPropertyRecord,
}

impl<'a> PackedPathRef<'a> {
    pub(super) fn new(
        path_id: PathId,
        path: &'a PackedPath,
        properties: PathPropertyRecord,
    ) -> Self {
        PackedPathRef {
            path_id,
            path,
            properties,
        }
    }

    pub(super) fn properties<'b>(&'b self) -> &'b PathPropertyRecord {
        &self.properties
    }
}

/// A representation of a step that's added to a path, that must be
/// inserted into the occurrences list and linked to the correct list
/// for the handle.
///
/// The path ID must be provided separately, and the `Handle` must be
/// transformed into a `NodeRecordId` so that the list for the node in
/// question can be identified.
pub type StepUpdate = crate::pathhandlegraph::StepUpdate<PathStepIx>;

pub struct PackedPathRefMut<'a> {
    pub path_id: PathId,
    pub path: &'a mut PackedPath,
    pub properties: PathPropertyRecord,
}

impl PathStep for (PathStepIx, PackedStep) {
    fn handle(&self) -> Handle {
        self.1.handle
    }
}

impl<'a> PathBase for PackedPathRef<'a> {
    type Step = (PathStepIx, PackedStep);

    type StepIx = PathStepIx;
}

impl<'a> PathBase for PackedPathRefMut<'a> {
    type Step = (PathStepIx, PackedStep);

    type StepIx = PathStepIx;
}

impl<'a> PathRef for PackedPathRef<'a> {
    type Steps = list::Iter<'a, PackedPath>;

    fn steps(self) -> Self::Steps {
        let head = self.properties.head;
        let tail = self.properties.tail;
        self.path.iter(head, tail)
    }

    fn len(self) -> usize {
        self.path.steps.len()
    }

    fn circular(self) -> bool {
        self.properties.circular
    }

    fn first_step(self) -> Self::Step {
        let head = self.properties.head;
        let step = self.path.get_step(head);
        (head, step)
    }

    fn last_step(self) -> Self::Step {
        let tail = self.properties.tail;
        let step = self.path.get_step(tail);
        (tail, step)
    }

    fn next_step(self, step: Self::Step) -> Option<Self::Step> {
        let next = self.path.next_step(step.0)?;
        let next_step = self.path.get_step(next);
        Some((next, next_step))
    }

    fn prev_step(self, step: Self::Step) -> Option<Self::Step> {
        let prev = self.path.prev_step(step.0)?;
        let prev_step = self.path.get_step(prev);
        Some((prev, prev_step))
    }
}

impl<'a> PackedPathRefMut<'a> {
    pub(super) fn new(
        path_id: PathId,
        path: &'a mut PackedPath,
        properties: PathPropertyRecord,
    ) -> Self {
        // let updates = PathUpdate::new(&properties);
        PackedPathRefMut {
            path_id,
            path,
            properties,
            // updates,
        }
    }

    /*
    #[must_use]
    pub(super) fn append_handles<I>(&mut self, iter: I) -> Vec<StepUpdate>
    where
        I: IntoIterator<Item = Handle>,
    {
        let mut tail = self.properties.tail;

        let mut iter = iter.into_iter();

        let first_step = if let Some(first) = iter.next() {
            self.append_handle(first)
        } else {
            return Vec::new();
        };

        let mut new_steps = iter
            .into_iter()
            .map(|handle| {
                let step = self.path.append_handle(handle);

                // add back link from new step to old tail
                let new_prev_ix = step.to_record_ix(2, 0).unwrap();
                self.path.links.set_pack(new_prev_ix, tail);

                // just in case the path was empty, set the head as well
                if self.updates.head.is_null() {
                    self.updates.head = step;
                }

                if let Some(tail_next_ix) = tail.to_record_ix(2, 1) {
                    // add forward link from old tail to new step
                    self.path.links.set_pack(tail_next_ix, step);
                }
                tail = step;

                StepUpdate { handle, step }
            })
            .collect::<Vec<_>>();

        self.updates.tail = tail;

        new_steps.push(first_step);

        new_steps
    }
    */

    #[must_use]
    pub(crate) fn append_handle(&mut self, handle: Handle) -> StepUpdate {
        let tail = self.properties.tail;
        let step = self.path.append_handle(handle);

        // add back link from new step to old tail
        let new_prev_ix = step.to_record_ix(2, 0).unwrap();
        self.path.links.set_pack(new_prev_ix, tail);

        // just in case the path was empty, set the head as well
        if self.properties.head.is_null() {
            self.properties.head = step;
        }

        if let Some(tail_next_ix) = tail.to_record_ix(2, 1) {
            // add forward link from old tail to new step
            self.path.links.set_pack(tail_next_ix, step);
        }
        // set the new tail
        self.properties.tail = step;

        let update = StepUpdate::Insert { handle, step };

        update
    }

    #[must_use]
    pub(crate) fn prepend_handle(&mut self, handle: Handle) -> StepUpdate {
        let head = self.properties.head;
        let step = self.path.append_handle(handle);

        // add forward link from new step to old head
        let new_next_ix = step.to_record_ix(2, 1).unwrap();
        self.path.links.set_pack(new_next_ix, head);

        // just in case the path was empty, set the tail as well
        if self.properties.tail.is_null() {
            self.properties.tail = step;
        }

        if let Some(head_prev_ix) = head.to_record_ix(2, 01) {
            // add back link from old head to new step
            self.path.links.set_pack(head_prev_ix, step);
        }
        // set the new head
        self.properties.head = step;

        let update = StepUpdate::Insert { handle, step };

        update
    }

    fn remove_step_at_index(
        &mut self,
        rem_step_ix: PathStepIx,
    ) -> Option<StepUpdate> {
        let head = self.properties.head;
        let tail = self.properties.tail;

        let handle = self.path.step_record(rem_step_ix)?;

        if tail == rem_step_ix {
            let (prev, _) = self.path.link_record(rem_step_ix)?;
            self.properties.tail = prev;
        }

        let new_head = self
            .path
            .iter_mut(head, tail)
            .remove_record_with(|step_ix, step| step_ix == rem_step_ix)?;

        self.properties.head = new_head;

        Some(StepUpdate::Remove {
            handle,
            step: rem_step_ix,
        })
    }
}

impl<'a> PathRefMut for PackedPathRefMut<'a> {
    fn append_step(&mut self, handle: Handle) -> StepUpdate {
        self.append_handle(handle)
    }

    fn prepend_step(&mut self, handle: Handle) -> StepUpdate {
        self.prepend_handle(handle)
    }

    fn remove_step(&mut self, rem_step_ix: Self::StepIx) -> Option<StepUpdate> {
        self.remove_step_at_index(rem_step_ix)
    }

    fn set_circularity(&mut self, circular: bool) {
        self.properties.circular = circular;
    }
}

impl<'a, 'b> PathRefMut for &'a mut PackedPathRefMut<'b> {
    fn append_step(&mut self, handle: Handle) -> StepUpdate {
        self.append_handle(handle)
    }

    fn prepend_step(&mut self, handle: Handle) -> StepUpdate {
        self.prepend_handle(handle)
    }
    fn remove_step(&mut self, step: Self::StepIx) -> Option<StepUpdate> {
        self.remove_step_at_index(step)
    }

    fn set_circularity(&mut self, circular: bool) {
        self.properties.circular = circular;
    }
}
