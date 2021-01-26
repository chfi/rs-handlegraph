use std::num::NonZeroUsize;

use fnv::FnvHashMap;

use crossbeam_channel::Sender;

use crate::{
    handle::{Handle, NodeId},
    packed::*,
    pathhandlegraph::{MutPath, PathBase, PathId, PathStep, PathSteps},
};

use crate::packedgraph::{
    defragment::Defragment,
    graph::NARROW_PAGE_WIDTH,
    index::list::{self, PackedDoubleList, PackedList, PackedListMut},
};

use super::{properties::*, OneBasedIndex, RecordIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StepPtr(Option<NonZeroUsize>);

crate::impl_space_usage_stack_newtype!(StepPtr);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PathLinkRecordIx(usize);

crate::impl_one_based_index!(StepPtr);

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

/// A reified record of a step in a StepList, with `handle` taken
/// from the `steps` list, and `prev` & `next` taking from the `links`
/// list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedStep {
    pub handle: Handle,
    pub prev: StepPtr,
    pub next: StepPtr,
}

#[derive(Debug, Clone)]
pub struct StepList {
    pub(crate) steps: RobustPagedIntVec,
    pub(crate) links: RobustPagedIntVec,
    pub(crate) removed_steps: usize,
    pub(crate) path_deleted: bool,
}

/// A representation of a step that's added to a path, that must be
/// inserted into the occurrences list and linked to the correct list
/// for the handle.
///
/// The path ID must be provided separately, and the `Handle` must be
/// transformed into a `NodeRecordId` so that the list for the node in
/// question can be identified.
pub type StepUpdate = crate::pathhandlegraph::StepUpdate<StepPtr>;

crate::impl_space_usage!(StepList, [steps, links]);

impl Default for StepList {
    fn default() -> Self {
        Self {
            steps: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(NARROW_PAGE_WIDTH),
            removed_steps: 0,
            path_deleted: false,
        }
    }
}

impl StepList {
    #[inline]
    pub fn len(&self) -> usize {
        if self.path_deleted {
            0
        } else {
            self.steps.len() - self.removed_steps
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub(crate) fn storage_len(&self) -> usize {
        self.steps.len()
    }

    pub(crate) fn append_handle_record(
        &mut self,
        handle: Handle,
        prev: u64,
        next: u64,
    ) -> StepPtr {
        let new_ix = StepPtr::from_zero_based(self.storage_len());
        self.steps.append(handle.pack());
        self.links.append(prev);
        self.links.append(next);
        new_ix
    }

    fn link_record(&self, ix: StepPtr) -> Option<(StepPtr, StepPtr)> {
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;
        let prev = self.links.get_unpack(link_ix.record_ix(0));
        let next = self.links.get_unpack(link_ix.record_ix(1));
        Some((prev, next))
    }

    fn step_record(&self, ix: StepPtr) -> Option<Handle> {
        let step_ix = ix.to_record_start(1)?;
        let step = self.steps.get_unpack(step_ix);
        Some(step)
    }

    pub(crate) fn get_step(&self, ix: StepPtr) -> Option<PackedStep> {
        let handle = self.step_record(ix)?;
        let (prev, next) = self.link_record(ix)?;
        Some(PackedStep { handle, prev, next })
    }

    fn get_step_unchecked(&self, ix: StepPtr) -> PackedStep {
        self.get_step(ix).unwrap()
    }

    pub(super) fn prev_step(&self, ix: StepPtr) -> Option<StepPtr> {
        let link_ix = ix.to_record_ix(2, 0)?;
        let link = self.links.get_unpack(link_ix);
        Some(link)
    }

    pub(super) fn next_step(&self, ix: StepPtr) -> Option<StepPtr> {
        let link_ix = ix.to_record_ix(2, 1)?;
        let link = self.links.get_unpack(link_ix);
        Some(link)
    }

    pub(super) fn insert_after(
        &mut self,
        ix: StepPtr,
        handle: Handle,
    ) -> Option<StepPtr> {
        let new_ix = StepPtr::from_zero_based(self.steps.len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.as_integer());

        let ix_next: StepPtr = self.links.get_unpack(link_ix.record_ix(1));

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

    #[allow(dead_code)]
    pub(super) fn insert_before(
        &mut self,
        ix: StepPtr,
        handle: Handle,
    ) -> Option<StepPtr> {
        let new_ix = StepPtr::from_zero_based(self.storage_len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.pack());

        let ix_prev: StepPtr = self.links.get_unpack(link_ix.record_ix(0));

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
        head: StepPtr,
        tail: StepPtr,
    ) -> list::Iter<'_, StepList> {
        list::Iter::new_double(self, head, tail)
    }

    pub(crate) fn iter_mut(
        &mut self,
        head: StepPtr,
        tail: StepPtr,
    ) -> list::IterMut<'_, StepList> {
        list::IterMut::new_double(self, head, tail)
    }

    pub(crate) fn transform_steps<F>(&mut self, mut transform: F)
    where
        F: FnMut(NodeId) -> NodeId,
    {
        let length = self.storage_len();

        for ix in 0..length {
            let handle: Handle = self.steps.get_unpack(ix);
            let n_id = handle.id();
            if !n_id.is_zero() {
                let new_handle =
                    Handle::pack(transform(n_id), handle.is_reverse());
                self.steps.set_pack(ix, new_handle);
            }
        }
    }
}

impl PackedList for StepList {
    type ListPtr = StepPtr;
    type ListRecord = PackedStep;

    #[inline]
    fn next_pointer(rec: &PackedStep) -> StepPtr {
        rec.next
    }

    #[inline]
    fn get_record(&self, ptr: StepPtr) -> Option<PackedStep> {
        let link_ix = PathLinkRecordIx::from_one_based_ix(ptr)?;
        let prev = self.links.get_unpack(link_ix.record_ix(0));
        let next = self.links.get_unpack(link_ix.record_ix(1));

        let step_ix = ptr.to_record_start(1)?;
        let handle = self.steps.get_unpack(step_ix);

        Some(PackedStep { prev, next, handle })
    }
}

impl PackedListMut for StepList {
    type ListLink = (StepPtr, StepPtr);

    #[inline]
    fn get_record_link(record: &PackedStep) -> Self::ListLink {
        (record.prev, record.next)
    }

    #[inline]
    fn link_next(link: Self::ListLink) -> StepPtr {
        link.1
    }

    #[inline]
    fn remove_at_pointer(&mut self, ptr: StepPtr) -> Option<Self::ListLink> {
        let step_ix = ptr.to_record_ix(1, 0)?;

        let prev_ix = ptr.to_record_ix(2, 0)?;
        let next_ix = prev_ix + 1;

        let prev_ptr: StepPtr = self.links.get_unpack(prev_ix);
        let next_ptr: StepPtr = self.links.get_unpack(next_ix);

        match (prev_ptr.to_record_ix(2, 1), next_ptr.to_record_ix(2, 0)) {
            (Some(p_ix), Some(n_ix)) => {
                // update both pointers
                self.links.set_pack(p_ix, next_ptr);
                self.links.set_pack(n_ix, prev_ptr);
            }
            (None, Some(n_ix)) => {
                // set next's previous pointer to zero
                self.links.set_pack(n_ix, StepPtr::null());
            }
            (Some(p_ix), None) => {
                // set prev's next pointer to zero
                self.links.set_pack(p_ix, StepPtr::null());
            }
            (None, None) => (),
        }

        self.steps.set(step_ix, 0);

        self.links.set(prev_ix, 0);
        self.links.set(next_ix, 0);

        self.removed_steps += 1;

        Some((prev_ptr, next_ptr))
    }

    #[inline]
    fn remove_next(&mut self, ptr: StepPtr) -> Option<()> {
        let (_prev, next) = self.link_record(ptr)?;
        let _ = self.remove_at_pointer(next)?;

        Some(())
    }
}

impl PackedDoubleList for StepList {
    #[inline]
    fn prev_pointer(rec: &PackedStep) -> StepPtr {
        rec.prev
    }
}

impl Defragment for StepList {
    type Updates = FnvHashMap<StepPtr, StepPtr>;

    fn defragment(&mut self) -> Option<Self::Updates> {
        if self.removed_steps == 0 || self.path_deleted {
            return None;
        }

        let total_len = self.storage_len();
        let new_length = self.len();

        let mut step_ix_map: FnvHashMap<StepPtr, StepPtr> =
            FnvHashMap::default();

        let mut new_steps = RobustPagedIntVec::new(NARROW_PAGE_WIDTH);
        let mut new_links = RobustPagedIntVec::new(NARROW_PAGE_WIDTH);
        new_steps.reserve(new_length);
        new_links.reserve(new_length * 2);

        let mut next_ix = 0usize;

        for ix in 0..total_len {
            let handle = self.steps.get(ix);

            if handle != 0 {
                let step_ix = StepPtr::from_zero_based(ix);
                let new_ix = StepPtr::from_zero_based(next_ix);

                new_steps.append(handle);

                let link_ix = ix * 2;
                let prev: StepPtr = self.links.get_unpack(link_ix);
                let next: StepPtr = self.links.get_unpack(link_ix + 1);
                new_links.append(prev.pack());
                new_links.append(next.pack());

                step_ix_map.insert(step_ix, new_ix);

                next_ix += 1;
            }
        }

        for ix in 0..new_length {
            let link_ix = ix * 2;
            let old_prev: StepPtr = new_links.get_unpack(link_ix);
            let old_next: StepPtr = new_links.get_unpack(link_ix + 1);

            if !old_prev.is_null() {
                let prev = step_ix_map.get(&old_prev).unwrap();
                new_links.set_pack(link_ix, *prev);
            }

            if !old_next.is_null() {
                let next = step_ix_map.get(&old_next).unwrap();
                new_links.set_pack(link_ix + 1, *next);
            }
        }

        self.steps = new_steps;
        self.links = new_links;
        self.removed_steps = 0;

        Some(step_ix_map)
    }
}

/// Helper trait, together with `AsStepsMut` for abstracting over
/// shared and mutable references in the type parameter of
/// `PackedPath`
pub trait AsStepsRef {
    fn steps_ref(&self) -> &StepList;
}

/// Helper trait, together with `AsStepsRef` for abstracting over
/// shared and mutable references in the type parameter of
/// `PackedPath`
pub trait AsStepsMut: AsStepsRef {
    fn steps_mut(&mut self) -> &mut StepList;
}

/// Wrapper over a shared reference to a `StepList`
#[derive(Debug, Clone, Copy)]
pub struct StepListRef<'a>(&'a StepList);

/// Wrapper over a mutable reference to a `StepList`
pub struct StepListMut<'a>(&'a mut StepList);

impl<'a> std::ops::Deref for StepListRef<'a> {
    type Target = StepList;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> std::ops::Deref for StepListMut<'a> {
    type Target = StepList;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> std::ops::DerefMut for StepListMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> AsStepsRef for StepListRef<'a> {
    fn steps_ref(&self) -> &StepList {
        self.0
    }
}

impl<'a> AsStepsRef for StepListMut<'a> {
    fn steps_ref(&self) -> &StepList {
        self.0
    }
}

impl<'a> AsStepsMut for StepListMut<'a> {
    fn steps_mut(&mut self) -> &mut StepList {
        self.0
    }
}

/// An encapsulation of a packed path, represented as a list of steps,
/// and with various properties that cannot be *stored* in the same
/// place as the steps list, but are semantically associated with a
/// path, and are needed for querying and manipulating the path.
///
/// The parameter `T: AsStepsRef` lets us use this one type for both
/// immutable and mutable path references.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackedPath<T: AsStepsRef> {
    pub(crate) path_id: PathId,
    pub(crate) deleted_steps: usize,
    pub(crate) head: StepPtr,
    pub(crate) tail: StepPtr,
    pub(crate) circular: bool,
    pub(crate) path: T,
}

pub type PackedPathRef<'a> = PackedPath<StepListRef<'a>>;
pub type PackedPathMut<'a> = PackedPath<StepListMut<'a>>;

// impl<T: AsStepsRef> {
// }

impl<'a> PackedPathRef<'a> {
    /// Constructs a new `PackedPath` holding a shared reference to
    /// its path steps.
    pub(crate) fn new_ref(
        path_id: PathId,
        path: &'a StepList,
        properties: &PathPropertyRecord,
    ) -> Self {
        Self {
            path_id,
            path: StepListRef(path),
            deleted_steps: 0,
            head: properties.head,
            tail: properties.tail,
            circular: properties.circular,
        }
    }
}

impl<'a> PackedPathMut<'a> {
    /// Constructs a new `PackedPath` holding a mutable reference to
    /// its path steps.
    pub(crate) fn new_mut(
        path_id: PathId,
        path: &'a mut StepList,
        properties: &PathPropertyRecord,
    ) -> Self {
        Self {
            path_id,
            path: StepListMut(path),
            deleted_steps: 0,
            head: properties.head,
            tail: properties.tail,
            circular: properties.circular,
        }
    }

    #[inline]
    pub fn append_handle_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        handle: Handle,
    ) -> StepPtr {
        let step_update = self.append_handle(handle);
        let step = step_update.step();
        sender
            .send((self.path_id, step_update))
            .unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });
        step
    }

    #[inline]
    pub fn append_handles_iter_chn<I>(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        mut iter: I,
    )
    // ) -> Vec<StepPtr>
    where
        I: Iterator<Item = Handle>,
    {
        let steps_page_size = self.path.steps_ref().steps.page_size();
        let links_page_size = self.path.steps_ref().links.page_size();

        let mut steps_buf: Vec<u64> = Vec::with_capacity(steps_page_size);
        let mut links_buf: Vec<u64> = Vec::with_capacity(links_page_size);
        let mut page_buf: Vec<u64> = Vec::with_capacity(steps_page_size);

        let mut cur_ptr = self.path.steps_ref().storage_len() + 1;

        if self.head.is_null() {
            self.head = StepPtr::from_one_based(cur_ptr);
        }

        let steps_mut = self.path.steps_mut();

        while let Some(handle) = iter.next() {
            steps_buf.push(handle.pack());

            links_buf.push(StepPtr::from_one_based(cur_ptr - 1).pack());
            links_buf.push(StepPtr::from_one_based(cur_ptr + 1).pack());

            let path_id = self.path_id;

            sender.send((
                self.path_id,
                StepUpdate::Insert {
                    handle,
                    step: StepPtr::from_one_based(cur_ptr),
                },
            )).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                path_id.0, cur_ptr
            );
            });

            if steps_buf.len() >= steps_page_size {
                steps_mut.steps.append_pages(&mut page_buf, &steps_buf);
                steps_buf.clear();
            }

            if links_buf.len() >= links_page_size {
                steps_mut.links.append_pages(&mut page_buf, &links_buf);
                links_buf.clear();
            }

            cur_ptr += 1;
        }

        if !steps_buf.is_empty() {
            self.path
                .steps_mut()
                .steps
                .append_pages(&mut page_buf, &steps_buf);
            steps_buf.clear();
        }

        if !links_buf.is_empty() {
            self.path
                .steps_mut()
                .links
                .append_pages(&mut page_buf, &links_buf);
            links_buf.clear();
        }

        let links_len = self.path.steps_ref().links.len();
        self.path.steps_mut().links.set(links_len - 1, 0);

        self.tail = StepPtr::from_one_based(cur_ptr - 1);
    }

    #[inline]
    pub fn prepend_handle_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        handle: Handle,
    ) -> StepPtr {
        let step_update = self.prepend_handle(handle);
        let step = step_update.step();
        sender.send((self.path_id, step_update)).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });
        step
    }

    #[inline]
    pub fn insert_handle_after_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        step: StepPtr,
        handle: Handle,
    ) -> Option<StepPtr> {
        let step_update = if step == self.tail {
            Some(self.append_step(handle))
        } else {
            let step = self.path.steps_mut().insert_after(step, handle)?;
            Some(StepUpdate::Insert { handle, step })
        }?;

        let step = step_update.step();
        sender.send((self.path_id, step_update)).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });
        Some(step)
    }

    #[inline]
    pub fn remove_step_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        step: StepPtr,
    ) -> bool {
        if let Some(step_update) = self.remove_step_at_index(step) {
            let step = step_update.step();
            sender.send((self.path_id, step_update)).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn rewrite_segment_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        from: StepPtr,
        to: StepPtr,
        new_segment: &[Handle],
    ) -> Option<(StepPtr, StepPtr)> {
        if new_segment.is_empty() {
            return None;
        }

        // make sure both steps actually exist in this path
        let (from_step, to_step) = {
            let steps = self.path.steps_ref();
            let from_step = steps.get_step(from)?;
            let to_step = steps.get_step(to)?;
            if from_step.handle.pack() == 0 || to_step.handle.pack() == 0 {
                return None;
            }
            (from_step, to_step)
        };

        // clear the steps to be removed, and push their corresponding
        // step updates
        {
            let steps = self.path.steps_mut();
            let mut to_remove = steps.iter(from, to).collect::<Vec<_>>();
            to_remove.pop();

            for (ptr, step) in to_remove.into_iter() {
                let path_id = self.path_id;
                sender.send((
                    self.path_id,
                    StepUpdate::Remove {
                        step: ptr,
                        handle: step.handle,
                    },
                )).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                path_id.0, ptr.pack()
            );
            });

                let step_ix = ptr.to_record_ix(1, 0)?;
                let link_ix = ptr.to_record_ix(2, 0)?;

                steps.steps.set(step_ix, 0);
                steps.links.set(link_ix, 0);
                steps.links.set(link_ix + 1, 0);
                steps.removed_steps += 1;
            }
        }

        let mut handles = new_segment.iter();
        let first_handle = *handles.next()?;

        // first added step, i.e. first handle in the provided slice
        let start = {
            let update = if from_step.prev.is_null() {
                self.prepend_step(first_handle)
            } else {
                self.insert_step_after(from_step.prev, first_handle)?
            };
            let step = update.step();
            sender.send((self.path_id, update)).unwrap();
            step
        };

        let mut last = start;
        for &handle in handles {
            let update = self.insert_step_after(last, handle)?;
            last = update.step();
            sender.send((self.path_id, update)).unwrap();
        }

        // last added step, i.e. last handle in the provided slice
        let end = last;

        let steps = self.path.steps_mut();

        // update the next-link of the step before the rewritten segment
        if let Some(ix) = from_step.prev.to_record_ix(2, 1) {
            steps.links.set_pack(ix, start);
        }
        // update the prev-link of the step after the rewritten segment
        if let Some(ix) = to_step.prev.to_record_ix(2, 0) {
            steps.links.set_pack(ix, end);
        }
        // update the next-link of the last step in the new segment
        if let Some(ix) = end.to_record_ix(2, 1) {
            steps.links.set_pack(ix, to);
        }

        Some((start, end))
    }

    #[inline]
    pub fn flip_step_orientation_chn(
        &mut self,
        sender: &mut Sender<(PathId, StepUpdate)>,
        step: StepPtr,
    ) -> Option<()> {
        let step_rec_ix = step.to_record_start(1)?;
        let handle: Handle =
            self.path.steps_mut().steps.get_unpack(step_rec_ix);

        self.path
            .steps_mut()
            .steps
            .set_pack(step_rec_ix, handle.flip());

        sender.send((self.path_id, StepUpdate::Remove { handle, step })).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });

        sender.send((
            self.path_id,
            StepUpdate::Insert {
                handle: handle.flip(),
                step,
            },
        )).unwrap_or_else(|_| {
                panic!(
                "Error sending update for path {}, step {} in append_handle_chn",
                self.path_id.0, step.pack()
            );
            });

        Some(())
    }
}

impl<T> PackedPath<T>
where
    T: AsStepsMut,
{
    #[must_use]
    pub(crate) fn append_handle(&mut self, handle: Handle) -> StepUpdate {
        let tail = self.tail;

        let step = self.path.steps_mut().append_handle_record(
            handle,
            self.tail.pack(),
            0,
        );

        // add back link from new step to old tail
        let new_prev_ix = step.to_record_ix(2, 0).unwrap();
        self.path.steps_mut().links.set_pack(new_prev_ix, tail);

        // just in case the path was empty, set the head as well
        if self.head.is_null() {
            self.head = step;
        }

        if let Some(tail_next_ix) = tail.to_record_ix(2, 1) {
            // add forward link from old tail to new step
            self.path.steps_mut().links.set_pack(tail_next_ix, step);
        }
        // set the new tail
        self.tail = step;

        StepUpdate::Insert { handle, step }
    }

    #[must_use]
    pub(crate) fn prepend_handle(&mut self, handle: Handle) -> StepUpdate {
        let head = self.head;

        let step =
            self.path
                .steps_mut()
                .append_handle_record(handle, 0, head.pack());

        // add forward link from new step to old head
        let new_next_ix = step.to_record_ix(2, 1).unwrap();
        self.path.steps_mut().links.set_pack(new_next_ix, head);

        // just in case the path was empty, set the tail as well
        if self.tail.is_null() {
            self.tail = step;
        }

        if let Some(head_prev_ix) = head.to_record_ix(2, 0) {
            // add back link from old head to new step
            self.path.steps_mut().links.set_pack(head_prev_ix, step);
        }
        // set the new head
        self.head = step;

        StepUpdate::Insert { handle, step }
    }

    pub(crate) fn remove_step_at_index(
        &mut self,
        rem_step_ix: StepPtr,
    ) -> Option<StepUpdate> {
        let head = self.head;
        let tail = self.tail;

        let handle = self.path.steps_mut().step_record(rem_step_ix)?;

        if tail == rem_step_ix {
            let (prev, _) = self.path.steps_mut().link_record(rem_step_ix)?;
            self.tail = prev;
        }

        let new_head = self
            .path
            .steps_mut()
            .iter_mut(head, tail)
            .remove_record_with(|step_ix, _step| step_ix == rem_step_ix)?;

        self.head = new_head;

        self.deleted_steps += 1;

        Some(StepUpdate::Remove {
            handle,
            step: rem_step_ix,
        })
    }

    pub(crate) fn flip_step_orientation(
        &mut self,
        step: StepPtr,
    ) -> Option<Vec<StepUpdate>> {
        let step_rec_ix = step.to_record_start(1)?;
        let handle: Handle =
            self.path.steps_mut().steps.get_unpack(step_rec_ix);
        self.path
            .steps_mut()
            .steps
            .set_pack(step_rec_ix, handle.flip());
        Some(vec![
            StepUpdate::Remove { handle, step },
            StepUpdate::Insert {
                handle: handle.flip(),
                step,
            },
        ])
    }

    fn rewrite_segment_impl(
        &mut self,
        from: StepPtr,
        to: StepPtr,
        new_segment: &[Handle],
    ) -> Option<(StepPtr, StepPtr, Vec<StepUpdate>)> {
        // if the head and/or tail are included in the overwritten segment
        let includes_head = self.head == from;
        let includes_tail = to.is_null();

        // the `from` step must exist in the path
        let from_step: PackedStep = self.path.steps_ref().get_record(from)?;
        let before_from = from_step.prev;

        // get the steps to be removed, while checking that `from`
        // exists in the path, and that `to`, if it's not null, comes
        // after `from`
        let to_remove = {
            let steps = self.path.steps_mut();

            let mut to_remove = steps.iter(from, to).collect::<Vec<_>>();

            // if the `to` `StepPtr` is null, we'll rewrite all steps
            // after `from`
            if !to.is_null() {
                // the provided range is end-exclusive, so we pop the
                // last entry
                if Some(to) != to_remove.pop().map(|(ptr, _)| ptr) {
                    // if we're not rewriting to the end of the path, the
                    // last entry of `to_remove` should be the provided
                    // end of the range -- if not, `to` either doesn't
                    // exist in the path, or it's before `from`, and we
                    // signal an error with `None`
                    return None;
                }
            }

            to_remove
        };

        let mod_prev =
            |steps: &mut StepList, step: StepPtr, new_prev: StepPtr| {
                step.to_record_ix(2, 0)
                    .map(|ix| steps.links.set_pack(ix, new_prev));
            };

        let mod_next =
            |steps: &mut StepList, step: StepPtr, new_next: StepPtr| {
                step.to_record_ix(2, 1)
                    .map(|ix| steps.links.set_pack(ix, new_next));
            };

        let link_pair =
            |steps: &mut StepList, left: StepPtr, right: StepPtr| {
                left.to_record_ix(2, 1)
                    .map(|ix| steps.links.set_pack(ix, right));
                right
                    .to_record_ix(2, 0)
                    .map(|ix| steps.links.set_pack(ix, left));
            };

        // clear the steps to be removed, push their removal updates,
        // and store the new range to be returned

        let mut updates: Vec<StepUpdate> =
            Vec::with_capacity(new_segment.len());

        {
            let steps = self.path.steps_mut();
            for (ptr, step) in to_remove {
                updates.push(StepUpdate::Remove {
                    step: ptr,
                    handle: step.handle,
                });

                let step_ix = ptr.to_record_ix(1, 0)?;
                let link_ix = ptr.to_record_ix(2, 0)?;

                steps.steps.set(step_ix, 0);
                steps.links.set(link_ix, 0);
                steps.links.set(link_ix + 1, 0);
                steps.removed_steps += 1;
            }

            // Update the links between the remaining steps so that
            // everything stays connected correctly as the new steps
            // are added
            match (includes_head, includes_tail) {
                // if neither head nor tail are affected, we need to
                // link the steps on either side of the removed range
                (false, false) => {
                    link_pair(steps, before_from, to);
                }
                // if only the tail is affected, we need to set the
                // `next` pointer on the new tail to null
                (false, true) => {
                    self.tail = before_from;
                    mod_next(steps, self.tail, StepPtr::null());
                }
                // if only the head is affected, we need to set the
                // `prev` pointer on the new head to null
                (true, false) => {
                    self.head = to;
                    mod_prev(steps, self.head, StepPtr::null());
                }
                // if both are affected, the path is empty
                (true, true) => {
                    self.head = StepPtr::null();
                    self.tail = StepPtr::null();
                }
            }
        }

        if new_segment.is_empty() {
            updates.shrink_to_fit();
            return Some((StepPtr::null(), StepPtr::null(), updates));
        }

        let mut handles = new_segment.iter();
        // easier to append the rest of the steps if we add a bit of
        // logic to set the first step correctly
        let first_handle = *handles.next()?;

        // pointers to the first and last new steps, to be returned
        let (start, mut end) = {
            let first_update = if includes_head {
                self.prepend_step(first_handle)
            } else {
                self.insert_step_after(before_from, first_handle)?
            };
            let step = first_update.step();
            updates.push(first_update);
            (step, step)
        };

        for &handle in handles {
            let update = self.insert_step_after(end, handle)?;
            end = update.step();
            updates.push(update);
        }

        updates.shrink_to_fit();
        Some((start, end, updates))
    }
}

impl PathStep for (StepPtr, PackedStep) {
    fn handle(&self) -> Handle {
        self.1.handle
    }
}

/// Both `PackedPath<StepListRef<'a>>` and
/// `PackedPath<StepListMut<'a>>` implement `PathBase`
impl<T> PathBase for PackedPath<T>
where
    T: AsStepsRef,
{
    type Step = (StepPtr, PackedStep);

    type StepIx = StepPtr;

    #[inline]
    fn len(&self) -> usize {
        self.path.steps_ref().len()
    }

    #[inline]
    fn circular(&self) -> bool {
        self.circular
    }

    #[inline]
    fn step_at(&self, index: StepPtr) -> Option<(StepPtr, PackedStep)> {
        let step = self.path.steps_ref().get_step(index)?;
        Some((index, step))
    }

    #[inline]
    fn first_step(&self) -> Self::StepIx {
        self.head
    }

    #[inline]
    fn last_step(&self) -> Self::StepIx {
        self.tail
    }

    #[inline]
    fn next_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        let next = self.path.steps_ref().next_step(step)?;
        let next_step = self.path.steps_ref().get_step_unchecked(next);
        Some((next, next_step))
    }

    #[inline]
    fn prev_step(&self, step: Self::StepIx) -> Option<Self::Step> {
        let prev = self.path.steps_ref().prev_step(step)?;
        let prev_step = self.path.steps_ref().get_step_unchecked(prev);
        Some((prev, prev_step))
    }
}

impl<T> MutPath for PackedPath<T>
where
    T: AsStepsMut,
{
    fn append_step(&mut self, handle: Handle) -> StepUpdate {
        self.append_handle(handle)
    }

    fn append_steps_iter<I>(&mut self, mut iter: I) -> Vec<StepUpdate>
    where
        I: Iterator<Item = Handle>,
    {
        let steps_page_size = self.path.steps_ref().steps.page_size();
        let links_page_size = self.path.steps_ref().links.page_size();

        let mut steps_buf: Vec<u64> = Vec::with_capacity(steps_page_size);
        let mut links_buf: Vec<u64> = Vec::with_capacity(links_page_size);
        let mut page_buf: Vec<u64> = Vec::with_capacity(steps_page_size);

        let mut step_updates: Vec<StepUpdate> =
            Vec::with_capacity(steps_page_size);

        let mut cur_ptr = self.path.steps_ref().storage_len() + 1;

        if self.head.is_null() {
            self.head = StepPtr::from_one_based(cur_ptr);
        }

        let steps_mut = self.path.steps_mut();

        while let Some(handle) = iter.next() {
            steps_buf.push(handle.pack());

            links_buf.push(StepPtr::from_one_based(cur_ptr - 1).pack());
            links_buf.push(StepPtr::from_one_based(cur_ptr + 1).pack());

            step_updates.push(StepUpdate::Insert {
                handle,
                step: StepPtr::from_one_based(cur_ptr),
            });

            if steps_buf.len() >= steps_page_size {
                steps_mut.steps.append_pages(&mut page_buf, &steps_buf);
                steps_buf.clear();
            }

            if links_buf.len() >= links_page_size {
                steps_mut.links.append_pages(&mut page_buf, &links_buf);
                links_buf.clear();
            }

            cur_ptr += 1;
        }

        if !steps_buf.is_empty() {
            self.path
                .steps_mut()
                .steps
                .append_pages(&mut page_buf, &steps_buf);
            steps_buf.clear();
        }

        if !links_buf.is_empty() {
            self.path
                .steps_mut()
                .links
                .append_pages(&mut page_buf, &links_buf);
            links_buf.clear();
        }

        let links_len = self.path.steps_ref().links.len();
        self.path.steps_mut().links.set(links_len - 1, 0);

        let new_tail =
            StepPtr::from_one_based(self.path.steps_ref().steps.len());

        self.tail = new_tail;

        step_updates
    }

    fn prepend_step(&mut self, handle: Handle) -> StepUpdate {
        self.prepend_handle(handle)
    }

    fn insert_step_after(
        &mut self,
        ix: Self::StepIx,
        handle: Handle,
    ) -> Option<StepUpdate> {
        if ix == self.tail {
            Some(self.append_step(handle))
        } else {
            let step = self.path.steps_mut().insert_after(ix, handle)?;
            Some(StepUpdate::Insert { handle, step })
        }
    }

    fn remove_step(&mut self, rem_step_ix: Self::StepIx) -> Option<StepUpdate> {
        self.remove_step_at_index(rem_step_ix)
    }

    fn flip_step(&mut self, step: Self::StepIx) -> Option<Vec<StepUpdate>> {
        self.flip_step_orientation(step)
    }

    fn rewrite_segment(
        &mut self,
        from: Self::StepIx,
        to: Self::StepIx,
        new_segment: &[Handle],
    ) -> Option<(Self::StepIx, Self::StepIx, Vec<StepUpdate>)> {
        self.rewrite_segment_impl(from, to, new_segment)
    }

    fn set_circularity(&mut self, circular: bool) {
        self.circular = circular;
    }
}

impl<'a, T> PathSteps for &'a PackedPath<T>
where
    T: AsStepsRef,
{
    type Steps = list::Iter<'a, StepList>;

    fn steps(self) -> Self::Steps {
        let head = self.head;
        let tail = self.tail;
        self.path.steps_ref().iter(head, tail)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    impl<'a> PackedPathMut<'a> {
        pub(crate) fn add_some_steps(
            &mut self,
            max_id: &mut usize,
            count: usize,
            prepend: bool,
        ) -> Vec<StepUpdate> {
            let mut updates = Vec::new();

            if prepend {
                let mut ids = (0..count)
                    .into_iter()
                    .map(|i| *max_id + i + 1)
                    .collect::<Vec<_>>();

                ids.reverse();
                for id in ids.into_iter() {
                    let handle = Handle::pack(id, false);
                    updates.push(self.prepend_step(handle));
                }
            } else {
                for i in 0..count {
                    let id = *max_id + i + 1;
                    let handle = Handle::pack(id, false);
                    updates.push(self.append_step(handle));
                }
            }

            *max_id += count;
            updates
        }

        pub(crate) fn remove_some_steps(
            &mut self,
            count: usize,
            from_head: bool,
        ) -> Vec<StepUpdate> {
            let mut updates = Vec::new();

            if from_head {
                for _step in 0..count {
                    let step = self.head;
                    let update = self.remove_step_at_index(step).unwrap();

                    updates.push(update);
                }
            } else {
                let step_indices = self
                    .steps()
                    .rev()
                    .take(count + 1)
                    .map(|(ix, _)| ix)
                    .collect::<Vec<_>>();

                for step in step_indices.into_iter().take(count) {
                    let update = self.remove_step_at_index(step).unwrap();
                    updates.push(update);
                }
            }

            updates
        }

        pub(crate) fn insert_many_into_middle(
            &mut self,
            max_id: &mut usize,
            count: usize,
        ) -> Vec<StepUpdate> {
            let mut updates = Vec::new();
            let length = self.len();

            let middle = self
                .steps()
                .map(|(ix, _)| ix)
                .nth((length / 2) - 1)
                .unwrap();

            let mut last = middle;

            for i in 0..count {
                let id = *max_id + i + 1;
                let handle = Handle::pack(id, false);
                let update = self.insert_step_after(last, handle).unwrap();
                if let StepUpdate::Insert { step, .. } = update {
                    last = step;
                }
                updates.push(update);
            }

            *max_id += count;

            updates
        }
    }

    impl StepList {
        fn generate_from_length(length: usize) -> (StepList, usize) {
            let mut path = StepList::default();
            let mut head =
                path.append_handle_record(Handle::pack(1, false), 0, 0);
            for id in 2..=length {
                let handle = Handle::pack(id, false);
                head = path.insert_after(head, handle).unwrap();
            }
            (path, length)
        }

        fn add_gen_steps(
            &mut self,
            head: &mut StepPtr,
            tail: &mut StepPtr,
            max_id: &mut usize,
            prepend: bool,
            count: usize,
        ) -> Vec<StepUpdate> {
            let mut updates = Vec::new();
            if prepend {
                for i in 0..count {
                    let handle = Handle::pack(*max_id + i + 1, false);
                    let step = self.insert_before(*head, handle).unwrap();
                    *head = step;
                    updates.push(StepUpdate::Insert { handle, step })
                }
            } else {
                for i in 0..count {
                    let handle = Handle::pack(*max_id + i + 1, false);
                    let step = self.insert_after(*tail, handle).unwrap();
                    *tail = step;
                    updates.push(StepUpdate::Insert { handle, step })
                }
            }
            *max_id += count;
            updates
        }

        fn insert_into_middle(
            &mut self,
            head: &StepPtr,
            tail: &StepPtr,
            max_id: &mut usize,
        ) -> StepUpdate {
            let length = self.iter(*head, *tail).count();
            let middle = self
                .iter(*head, *tail)
                .map(|(ix, _)| ix)
                .nth((length / 2) - 1)
                .unwrap();
            let handle = Handle::pack(*max_id + 1, false);

            let step = self.insert_after(middle, handle).unwrap();

            *max_id += 1;

            StepUpdate::Insert { step, handle }
        }

        fn remove_gen_steps(
            &mut self,
            head: &mut StepPtr,
            tail: &mut StepPtr,
            from_head: bool,
            count: usize,
        ) -> Vec<StepUpdate> {
            let mut updates = Vec::new();
            if from_head {
                for _step in 0..count {
                    let step = *head;
                    let handle = self.step_record(step).unwrap();
                    let new_head = self
                        .iter_mut(*head, StepPtr::null())
                        .remove_record_with(|_, _| true)
                        .unwrap();

                    *head = new_head;
                    updates.push(StepUpdate::Remove { step, handle });
                }
            } else {
                let step_indices = self
                    .iter(*head, *tail)
                    .rev()
                    .take(count + 1)
                    .map(|(ix, _)| ix)
                    .collect::<Vec<_>>();
                let new_tail = *step_indices.last().unwrap();
                for step in step_indices.into_iter().take(count) {
                    let handle = self.step_record(step).unwrap();
                    self.iter_mut(*head, *tail)
                        .remove_record_with(|step_ix, _| step_ix == step);
                    updates.push(StepUpdate::Remove { step, handle });
                }
                *tail = new_tail;
            }

            updates
        }
    }

    #[test]
    fn generate_path() {
        let len = 10usize;
        let (path, _) = StepList::generate_from_length(len);
        let head = StepPtr::from_zero_based(0usize);
        let tail = StepPtr::from_zero_based(path.steps.len() - 1);

        for (step_ix, step) in path.iter(head, tail) {
            println!(
                "{:?}\t{:?}\t{:?}\t{:?}",
                step.handle, step.prev, step_ix, step.next
            );
        }
    }

    pub(crate) fn print_path(path: &StepList, head: StepPtr, tail: StepPtr) {
        println!("  Head: {:?}\tTail: {:?}", head, tail);
        println!("  {:5}  {:4}  {:4}  {:4}", "Index", "Node", "Prev", "Next");
        for (step_ix, step) in path.iter(head, tail) {
            println!(
                "  {:>5}  {:>4}  {:>4}  {:>4}",
                step_ix.pack(),
                u64::from(step.handle.id()),
                step.prev.pack(),
                step.next.pack()
            );
        }
        println!("  -----------");
    }

    pub(crate) fn print_path_vecs(path: &StepList) {
        println!("{:5}  {:4}  {:4}  {:4}", "Index", "Node", "Prev", "Next");
        for ix in 0..path.steps.len() {
            let handle: Handle = path.steps.get_unpack(ix);

            let l_ix = ix * 2;
            let prev: StepPtr = path.links.get_unpack(l_ix);
            let next: StepPtr = path.links.get_unpack(l_ix + 1);

            let index = ix + 1;

            let node = u64::from(handle.id());

            if node == 0 {
                println!("{:>5}  {:^16}", index, "!<Empty Record>!");
            } else {
                println!(
                    "{:>5}  {:>4}  {:>4}  {:>4}",
                    index,
                    u64::from(handle.id()),
                    prev.pack(),
                    next.pack()
                );
            }
        }
    }

    pub(crate) fn path_handles(
        path: &StepList,
        head: StepPtr,
        tail: StepPtr,
    ) -> Vec<Handle> {
        path.iter(head, tail).map(|(_, step)| step.handle).collect()
    }

    pub(crate) fn path_vectors(path: &StepList) -> Vec<(usize, u64, u64, u64)> {
        let mut results = Vec::new();

        for ix in 0..path.steps.len() {
            let handle: Handle = path.steps.get_unpack(ix);

            let l_ix = ix * 2;
            let prev: StepPtr = path.links.get_unpack(l_ix);
            let next: StepPtr = path.links.get_unpack(l_ix + 1);

            let index = ix + 1;

            results.push((
                index,
                u64::from(handle.id()),
                prev.pack(),
                next.pack(),
            ));
        }

        results
    }

    #[test]
    fn defrag_path() {
        let len = 4usize;
        let (mut path, mut max_id) = StepList::generate_from_length(len);

        let mut head = StepPtr::from_zero_based(0usize);
        let mut tail = StepPtr::from_zero_based(path.steps.len() - 1);

        // prepending two steps
        path.add_gen_steps(&mut head, &mut tail, &mut max_id, true, 2);

        // appending three steps
        path.add_gen_steps(&mut head, &mut tail, &mut max_id, false, 3);

        // remove three steps from start
        path.remove_gen_steps(&mut head, &mut tail, true, 3);

        // remove two steps from end
        path.remove_gen_steps(&mut head, &mut tail, false, 2);

        // insert into middle
        path.insert_into_middle(&head, &tail, &mut max_id);

        let new_steps = path_handles(&path, head, tail);

        let expected_steps = [2, 3, 10, 4, 7]
            .iter()
            .map(|x| Handle::pack(*x, false))
            .collect::<Vec<_>>();
        assert_eq!(new_steps, expected_steps);

        let path_vecs = path_vectors(&path);

        let expected_pre_defrag = vec![
            (1, 0, 0, 0),
            (2, 2, 0, 3),
            (3, 3, 2, 10),
            (4, 4, 10, 7),
            (5, 0, 0, 0),
            (6, 0, 0, 0),
            (7, 7, 4, 0),
            (8, 0, 0, 0),
            (9, 0, 0, 0),
            (10, 10, 3, 4),
        ];

        assert_eq!(path.removed_steps, 5);
        assert_eq!(path_vecs, expected_pre_defrag);

        let updates = path.defragment().unwrap();

        let head = *updates.get(&head).unwrap();
        let tail = *updates.get(&tail).unwrap();

        let mut updates = updates
            .into_iter()
            .map(|(k, v)| (k.pack(), v.pack()))
            .collect::<Vec<_>>();

        updates.sort();
        assert_eq!(updates, vec![(2, 1), (3, 2), (4, 3), (7, 4), (10, 5)]);

        let defrag_path_vecs = path_vectors(&path);
        assert_eq!(
            defrag_path_vecs,
            vec![
                (1, 2, 0, 2),
                (2, 3, 1, 5),
                (3, 4, 5, 4),
                (4, 7, 3, 0),
                (5, 10, 2, 3)
            ]
        );

        let new_steps = path_handles(&path, head, tail);
        assert_eq!(new_steps, expected_steps);
    }
}
