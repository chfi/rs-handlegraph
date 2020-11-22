use crate::{packed::*, pathhandlegraph::PathId};

use super::{
    super::graph::{NARROW_PAGE_WIDTH, WIDE_PAGE_WIDTH},
    StepPtr,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPropertyRecord {
    pub(crate) head: StepPtr,
    pub(crate) tail: StepPtr,
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

    pub(super) fn append_new(&mut self, circular: bool) {
        self.heads.append(0);
        self.tails.append(0);
        self.deleted.append(0);
        if circular {
            self.circular.append(1);
        } else {
            self.circular.append(0);
        }
        self.deleted_steps.append(0);
    }

    pub(super) fn append_record(&mut self, record: PathPropertyRecord) {
        self.heads.append(record.head.pack());
        self.tails.append(record.tail.pack());
        self.deleted.append(record.deleted.pack());
        self.circular.append(record.circular.pack());
        self.deleted_steps.append(record.deleted_steps.pack());
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
}
