#![allow(dead_code)]

// use crate::handle::{Direction, Edge, Handle, NodeId};

use super::super::graph::{NARROW_PAGE_WIDTH, WIDE_PAGE_WIDTH};

use crate::pathhandlegraph::PathId;

use crate::packed::*;

use super::PathStepIx;

pub type StepView<'a> = ViewRef<'a, PagedIntVec, PathStepIx>;
pub type BoolView<'a> = ViewRef<'a, PackedIntVec, bool>;
pub type UsizeView<'a> = ViewRef<'a, PackedIntVec, usize>;

pub type StepViewMut<'a> = ViewMut<'a, PagedIntVec, PathStepIx>;
pub type BoolViewMut<'a> = ViewMut<'a, PackedIntVec, bool>;
pub type UsizeViewMut<'a> = ViewMut<'a, PackedIntVec, usize>;

#[derive(Debug, Clone)]
pub struct PathPropertyRef<'a> {
    head: StepView<'a>,
    tail: StepView<'a>,
    deleted: BoolView<'a>,
    circular: BoolView<'a>,
    deleted_steps: UsizeView<'a>,
}

impl<'a> PathPropertyRef<'a> {
    pub(super) fn get_head(&self) -> PathStepIx {
        self.head.get()
    }
    pub(super) fn get_tail(&self) -> PathStepIx {
        self.tail.get()
    }
    pub(super) fn get_deleted(&self) -> bool {
        self.deleted.get()
    }
    pub(super) fn get_circular(&self) -> bool {
        self.circular.get()
    }
    pub(super) fn get_deleted_steps(&self) -> usize {
        self.deleted_steps.get()
    }
}

#[derive(Debug)]
pub struct PathPropertyMut<'a> {
    head: StepViewMut<'a>,
    tail: StepViewMut<'a>,
    deleted: BoolViewMut<'a>,
    circular: BoolViewMut<'a>,
    deleted_steps: UsizeViewMut<'a>,
}

impl<'a> PathPropertyMut<'a> {
    pub(super) fn get_head(&self) -> PathStepIx {
        self.head.get()
    }
    pub(super) fn get_tail(&self) -> PathStepIx {
        self.tail.get()
    }
    pub(super) fn get_deleted(&self) -> bool {
        self.deleted.get()
    }
    pub(super) fn get_circular(&self) -> bool {
        self.circular.get()
    }
    pub(super) fn get_deleted_steps(&self) -> usize {
        self.deleted_steps.get()
    }

    pub(super) fn set_head(&mut self, step: PathStepIx) {
        self.head.set(step)
    }
    pub(super) fn set_tail(&mut self, step: PathStepIx) {
        self.tail.set(step)
    }
    pub(super) fn set_deleted(&mut self, val: bool) {
        self.deleted.set(val)
    }
    pub(super) fn set_circular(&mut self, val: bool) {
        self.circular.set(val)
    }
    pub(super) fn set_deleted_steps(&mut self, steps: usize) {
        self.deleted_steps.set(steps)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPropertyRecord {
    pub(crate) head: PathStepIx,
    pub(crate) tail: PathStepIx,
    pub(crate) deleted: bool,
    pub(crate) circular: bool,
    pub(crate) deleted_steps: usize,
}

#[derive(Debug, Clone)]
pub struct PathProperties {
    pub(crate) heads: PagedIntVec,
    pub(crate) tails: PagedIntVec,
    pub(crate) deleted: PackedIntVec,
    pub(crate) circular: PackedIntVec,
    pub(crate) deleted_steps: PackedIntVec,
}

crate::impl_space_usage!(
    PathProperties,
    [heads, tails, deleted, circular, deleted_steps]
);

impl Default for PathProperties {
    fn default() -> PathProperties {
        Self {
            heads: PagedIntVec::new(WIDE_PAGE_WIDTH),
            tails: PagedIntVec::new(NARROW_PAGE_WIDTH),
            deleted: Default::default(),
            circular: Default::default(),
            deleted_steps: Default::default(),
        }
    }
}

impl PathProperties {
    pub(super) fn append_empty(&mut self) {
        self.heads.append(0);
        self.tails.append(0);
        self.deleted.append(0);
        self.circular.append(0);
        self.deleted_steps.append(0);
    }

    pub(super) fn append_record(&mut self, record: PathPropertyRecord) {
        self.heads.append(record.head.pack());
        self.tails.append(record.tail.pack());
        self.deleted.append(record.deleted.pack());
        self.circular.append(record.circular.pack());
        self.deleted_steps.append(record.deleted_steps.pack());
    }

    pub(super) fn len(&self) -> usize {
        self.heads.len()
    }

    pub(super) fn clear_record(&mut self, id: PathId) {
        let ix = id.0 as usize;
        self.heads.set(ix, 0);
        self.tails.set(ix, 0);
        self.deleted.set(ix, 0);
        self.circular.set(ix, 0);
        self.deleted_steps.set(ix, 0);
    }

    pub(super) fn get_record(&self, id: PathId) -> PathPropertyRecord {
        let ix = id.0 as usize;
        PathPropertyRecord {
            head: self.heads.get_unpack(ix),
            tail: self.tails.get_unpack(ix),
            deleted: self.deleted.get_unpack(ix),
            circular: self.circular.get_unpack(ix),
            deleted_steps: self.deleted_steps.get_unpack(ix),
        }
    }

    pub(super) fn record_ref(&self, id: PathId) -> PathPropertyRef<'_> {
        let ix = id.0 as usize;
        PathPropertyRef {
            head: self.heads.view::<PathStepIx>(ix),
            tail: self.tails.view::<PathStepIx>(ix),
            deleted: self.deleted.view::<bool>(ix),
            circular: self.circular.view::<bool>(ix),
            deleted_steps: self.deleted_steps.view::<usize>(ix),
        }
    }

    pub(super) fn record_mut(&mut self, id: PathId) -> PathPropertyMut<'_> {
        let ix = id.0 as usize;
        PathPropertyMut {
            head: self.heads.view_mut::<PathStepIx>(ix),
            tail: self.tails.view_mut::<PathStepIx>(ix),
            deleted: self.deleted.view_mut::<bool>(ix),
            circular: self.circular.view_mut::<bool>(ix),
            deleted_steps: self.deleted_steps.view_mut::<usize>(ix),
        }
    }

    /*
    fn set_record(&mut self, id: PathId, record: &PathPropertyRecord) -> bool {
        if id.0 >= self.len() as u64 {
            return false;
        }

        let ix = id.0 as usize;
        self.heads.set(ix, record.head_ptr.pack());
        self.tails.set(ix, record.tail_ptr.pack());
        self.deleted.set(ix, record.deleted.pack());
        self.circular.set(ix, record.circular.pack());
        self.deleted_steps.set(ix, record.deleted_steps.pack());
        true
    }

    fn get_record(&self, id: PathId) -> Option<PathPropertyRecord> {
        if id.0 >= self.len() as u64 {
            return None;
        }
        let ix = id.0 as usize;

        let head_ptr = self.heads.get_unpack(ix);
        let tail_ptr = self.tails.get_unpack(ix);
        let deleted = self.deleted.get_unpack(ix);
        let circular = self.circular.get_unpack(ix);
        let deleted_steps = self.deleted_steps.get_unpack(ix);

        Some(PathPropertyRecord {
            head_ptr,
            tail_ptr,
            deleted,
            circular,
            deleted_steps,
        })
    }
    */
}
