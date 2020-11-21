use std::num::NonZeroUsize;

use fnv::FnvHashMap;

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
    pub(crate) handle: Handle,
    pub(crate) prev: StepPtr,
    pub(crate) next: StepPtr,
}

#[derive(Debug, Clone)]
pub struct StepList {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
    pub(super) removed_steps: usize,
    pub(super) path_deleted: bool,
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
        self.steps.len() - self.removed_steps
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub(super) fn storage_len(&self) -> usize {
        self.steps.len()
    }

    pub(super) fn append_handle_record(&mut self, handle: Handle) -> StepPtr {
        let new_ix = StepPtr::from_zero_based(self.storage_len());
        self.steps.append(handle.pack());
        self.links.append(0);
        self.links.append(0);
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

    fn get_step(&self, ix: StepPtr) -> Option<PackedStep> {
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

    pub(crate) fn transform_steps<F>(&mut self, transform: F)
    where
        F: Fn(NodeId) -> NodeId,
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
}

impl<T> PackedPath<T>
where
    T: AsStepsMut,
{
    #[must_use]
    pub(crate) fn append_handle(&mut self, handle: Handle) -> StepUpdate {
        let tail = self.tail;
        let step = self.path.steps_mut().append_handle_record(handle);

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
        let step = self.path.steps_mut().append_handle_record(handle);

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
    fn first_step(&self) -> Self::Step {
        let head = self.head;
        let step = self.path.steps_ref().get_step_unchecked(head);
        (head, step)
    }

    #[inline]
    fn last_step(&self) -> Self::Step {
        let tail = self.tail;
        let step = self.path.steps_ref().get_step_unchecked(tail);
        (tail, step)
    }

    #[inline]
    fn next_step(&self, step: Self::Step) -> Option<Self::Step> {
        let next = self.path.steps_ref().next_step(step.0)?;
        let next_step = self.path.steps_ref().get_step_unchecked(next);
        Some((next, next_step))
    }

    #[inline]
    fn prev_step(&self, step: Self::Step) -> Option<Self::Step> {
        let prev = self.path.steps_ref().prev_step(step.0)?;
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
    ) -> Option<Vec<StepUpdate>> {
        unimplemented!();
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

    impl<'a> PackedPathRefMut<'a> {
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
                    let step = self.properties.head;
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
                let update = self.insert_step_after(last, handle);
                if let StepUpdate::Insert { step, .. } = update {
                    last = step;
                }
                updates.push(update);
            }

            *max_id += count;

            updates
        }
    }

    impl PackedPathSteps {
        fn generate_from_length(length: usize) -> (PackedPathSteps, usize) {
            let mut path = PackedPathSteps::default();
            let mut head = path.append_handle_record(Handle::pack(1, false));
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
        let (path, _) = PackedPathSteps::generate_from_length(len);
        let head = StepPtr::from_zero_based(0usize);
        let tail = StepPtr::from_zero_based(path.steps.len() - 1);

        for (step_ix, step) in path.iter(head, tail) {
            println!(
                "{:?}\t{:?}\t{:?}\t{:?}",
                step.handle, step.prev, step_ix, step.next
            );
        }
    }

    pub(crate) fn print_path(
        path: &PackedPathSteps,
        head: StepPtr,
        tail: StepPtr,
    ) {
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

    pub(crate) fn print_path_vecs(path: &PackedPathSteps) {
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
        path: &PackedPathSteps,
        head: StepPtr,
        tail: StepPtr,
    ) -> Vec<Handle> {
        path.iter(head, tail).map(|(_, step)| step.handle).collect()
    }

    pub(crate) fn path_vectors(
        path: &PackedPathSteps,
    ) -> Vec<(usize, u64, u64, u64)> {
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
        let (mut path, mut max_id) = PackedPathSteps::generate_from_length(len);

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
