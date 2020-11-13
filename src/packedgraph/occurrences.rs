#![allow(dead_code)]

#[allow(unused_imports)]
use crate::handle::{Handle, NodeId};

use super::graph::{NARROW_PAGE_WIDTH, WIDE_PAGE_WIDTH};

use std::num::NonZeroUsize;

use fnv::FnvHashMap;

#[allow(unused_imports)]
use super::{NodeRecordId, OneBasedIndex, PathStepIx, RecordIndex};

use super::list;
use super::list::{PackedList, PackedListMut};

use crate::pathhandlegraph::*;

use crate::packed::*;

use super::defragment::Defragment;

/// The index for a node path occurrence record. Valid indices are
/// natural numbers starting from 1, each denoting a *record*. A zero
/// denotes the end of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OccurListIx(Option<NonZeroUsize>);

crate::impl_one_based_index!(OccurListIx);
crate::impl_space_usage_stack_newtype!(OccurListIx);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OccurRecord {
    pub(crate) path_id: PathId,
    pub(crate) offset: PathStepIx,
    next: OccurListIx,
}

#[derive(Debug, Clone)]
pub struct NodeOccurrences {
    path_ids: PagedIntVec,
    node_occur_offsets: PagedIntVec,
    node_occur_next: PagedIntVec,
    removed_records: usize,
}

crate::impl_space_usage!(
    NodeOccurrences,
    [
        path_ids,
        node_occur_offsets,
        node_occur_next,
        removed_records
    ]
);

impl Default for NodeOccurrences {
    fn default() -> Self {
        Self {
            path_ids: PagedIntVec::new(WIDE_PAGE_WIDTH),
            node_occur_offsets: PagedIntVec::new(NARROW_PAGE_WIDTH),
            node_occur_next: PagedIntVec::new(NARROW_PAGE_WIDTH),
            removed_records: 0,
        }
    }
}

impl Defragment for NodeOccurrences {
    type Updates = FnvHashMap<OccurListIx, OccurListIx>;

    fn defragment(&mut self) -> Option<Self::Updates> {
        if self.removed_records == 0 {
            return None;
        }

        let total_len = self.path_ids.len();
        let kept_len = self.path_ids.len() - self.removed_records;

        let mut updates: Self::Updates = FnvHashMap::default();

        let mut path_ids = PagedIntVec::new(WIDE_PAGE_WIDTH);
        let mut node_occur_offsets = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let mut node_occur_next = PagedIntVec::new(NARROW_PAGE_WIDTH);
        path_ids.reserve(kept_len);
        node_occur_offsets.reserve(kept_len);
        node_occur_next.reserve(kept_len);

        let mut next_ix = 0usize;

        for ix in 0..total_len {
            let path_id: PathId = self.path_ids.get_unpack(ix);
            let offset: PathStepIx = self.node_occur_offsets.get_unpack(ix);
            let next: OccurListIx = self.node_occur_next.get_unpack(ix);

            // TODO a record could still be valid even if all fields are
            // zero... need to handle this in a better way
            if !(path_id.0 == 0 && offset.is_null() && next.is_null()) {
                let old_ix = OccurListIx::from_zero_based(ix);
                let new_ix = OccurListIx::from_zero_based(next_ix);
                updates.insert(old_ix, new_ix);

                path_ids.append(path_id.pack());
                node_occur_offsets.append(offset.pack());
                node_occur_next.append(next.pack());

                next_ix += 1;
            }
        }

        for ix in 0..kept_len {
            let old_next: OccurListIx = node_occur_next.get_unpack(ix);
            if !old_next.is_null() {
                let next = updates.get(&old_next).unwrap();
                node_occur_next.set_pack(ix, *next);
            }
        }

        Some(updates)
    }
}

impl NodeOccurrences {
    pub(super) fn append_record(&mut self) -> OccurListIx {
        let node_rec_ix = OccurListIx::from_zero_based(self.path_ids.len());

        self.path_ids.append(0);
        self.node_occur_offsets.append(0);
        self.node_occur_next.append(0);

        node_rec_ix
    }

    pub(super) fn append_entry(
        &mut self,
        path: PathId,
        offset: PathStepIx,
        next: OccurListIx,
    ) -> OccurListIx {
        let node_rec_ix = OccurListIx::from_zero_based(self.path_ids.len());

        self.path_ids.append(path.0 as u64);
        self.node_occur_offsets.append(offset.pack());
        self.node_occur_next.append(next.pack());

        node_rec_ix
    }

    pub(super) fn add_link(&mut self, from: OccurListIx, to: OccurListIx) {
        let from_ix = from.to_zero_based().unwrap();
        self.node_occur_next.set_pack(from_ix, to);
    }

    pub(super) fn prepend_occurrence(
        &mut self,
        ix: OccurListIx,
        path_id: PathId,
        offset: PathStepIx,
    ) {
        let ix = ix.to_zero_based().unwrap();

        let rec_ix = self.append_record();

        let next: OccurListIx = self.node_occur_next.get_unpack(ix);

        let rec_ix = rec_ix.to_zero_based().unwrap();

        self.path_ids.set_pack(rec_ix, path_id.0);
        self.node_occur_offsets.set_pack(rec_ix, offset);
        self.node_occur_next.set_pack(rec_ix, next);
    }

    pub(super) fn set_record(
        &mut self,
        ix: OccurListIx,
        path_id: PathId,
        offset: PathStepIx,
        next: OccurListIx,
    ) -> bool {
        if let Some(ix) = ix.to_zero_based() {
            if ix >= self.path_ids.len() {
                return false;
            }

            self.path_ids.set_pack(ix, path_id.0);
            self.node_occur_offsets.set_pack(ix, offset);
            self.node_occur_next.set_pack(ix, next);

            true
        } else {
            false
        }
    }

    pub(crate) fn iter(&self, head: OccurListIx) -> list::Iter<'_, Self> {
        list::Iter::new(self, head)
    }

    pub(crate) fn iter_mut(
        &mut self,
        head: OccurListIx,
    ) -> list::IterMut<'_, Self> {
        list::IterMut::new(self, head)
    }

    pub(crate) fn apply_path_updates(
        &mut self,
        updates: &FnvHashMap<
            PathId,
            (PathId, FnvHashMap<PathStepIx, PathStepIx>),
        >,
    ) {
        let total_len = self.path_ids.len();

        for ix in 0..total_len {
            let old_path_id: PathId = self.path_ids.get_unpack(ix);
            let (new_path_id, offset_map) = updates.get(&old_path_id).unwrap();

            let old_offset: PathStepIx = self.node_occur_offsets.get_unpack(ix);
            let new_offset = offset_map.get(&old_offset).unwrap();

            self.path_ids.set_pack(ix, *new_path_id);
            self.node_occur_offsets.set_pack(ix, *new_offset);
        }
    }
}

impl PackedList for NodeOccurrences {
    type ListPtr = OccurListIx;
    type ListRecord = OccurRecord;

    #[inline]
    fn next_pointer(rec: &OccurRecord) -> OccurListIx {
        rec.next
    }

    #[inline]
    fn get_record(&self, ix: OccurListIx) -> Option<OccurRecord> {
        let ix = ix.to_zero_based()?;
        if ix >= self.path_ids.len() {
            return None;
        }

        let path_id = PathId(self.path_ids.get(ix));
        let offset = self.node_occur_offsets.get_unpack(ix);
        let next = self.node_occur_next.get_unpack(ix);

        Some(OccurRecord {
            path_id,
            offset,
            next,
        })
    }

    #[inline]
    fn next_record(&self, rec: &OccurRecord) -> Option<OccurRecord> {
        self.get_record(rec.next)
    }
}

impl PackedListMut for NodeOccurrences {
    type ListLink = OccurListIx;

    #[inline]
    fn get_record_link(record: &OccurRecord) -> OccurListIx {
        record.next
    }

    #[inline]
    fn link_next(link: OccurListIx) -> OccurListIx {
        link
    }

    #[inline]
    fn remove_at_pointer(&mut self, ptr: OccurListIx) -> Option<OccurListIx> {
        let ix = ptr.to_zero_based()?;

        let next = self.node_occur_next.get_unpack(ix);

        self.path_ids.set(ix, 0);
        self.node_occur_offsets.set(ix, 0);
        self.node_occur_next.set(ix, 0);

        self.removed_records += 1;

        Some(next)
    }

    #[inline]
    fn remove_next(&mut self, ptr: OccurListIx) -> Option<()> {
        let ix = ptr.to_zero_based()?;

        let next_ptr = self.node_occur_next.get_unpack(ix);

        let new_next_ptr = self.remove_at_pointer(next_ptr)?;
        self.node_occur_next.set_pack(ix, new_next_ptr);

        Some(())
    }
}

pub struct OccurrencesIter<'a> {
    list_iter: list::Iter<'a, NodeOccurrences>,
}

impl<'a> OccurrencesIter<'a> {
    pub(crate) fn new(list_iter: list::Iter<'a, NodeOccurrences>) -> Self {
        Self { list_iter }
    }
}

impl<'a> Iterator for OccurrencesIter<'a> {
    type Item = (PathId, PathStepIx);

    fn next(&mut self) -> Option<Self::Item> {
        let (_occ_ix, occ_rec) = self.list_iter.next()?;
        let path_id = occ_rec.path_id;
        let step_ix = occ_rec.offset;
        Some((path_id, step_ix))
    }
}
