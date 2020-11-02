use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{AdditiveHandleGraph, MutableHandleGraph},
};

use fnv::FnvHashMap;

use std::num::NonZeroUsize;

use super::{
    NodeRecordId, OneBasedIndex, PackedList, PackedListIter, RecordIndex,
};

use crate::pathhandlegraph::*;

use crate::packed;
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
        ix.to_record_ix(Self::RECORD_WIDTH).map(PathLinkRecordIx)
    }

    #[inline]
    fn to_one_based_ix<I: OneBasedIndex>(self) -> I {
        I::from_record_ix(self.0, Self::RECORD_WIDTH)
    }

    #[inline]
    fn record_ix(self, offset: usize) -> usize {
        self.0 + offset
    }
}

pub struct PackedPath {
    steps: RobustPagedIntVec,
    links: RobustPagedIntVec,
}

impl PackedPath {
    pub(super) fn new() -> Self {
        Self {
            steps: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            links: RobustPagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub(super) fn insert_step(
        &mut self,
        ix: PathStepIx,
        handle: Handle,
    ) -> Option<PathStepIx> {
        let new_ix = PathStepIx::from_zero_based(self.len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.as_integer());

        self.links.append(ix.pack());
        let ix_next = self.links.get(link_ix.record_ix(1));
        self.links.append(ix_next.pack());

        self.links.set(link_ix.record_ix(1), new_ix.pack());

        Some(new_ix)
    }

    pub(super) fn insert_step_before(
        &mut self,
        ix: PathStepIx,
        handle: Handle,
    ) -> Option<PathStepIx> {
        let new_ix = PathStepIx::from_zero_based(self.len());
        let link_ix = PathLinkRecordIx::from_one_based_ix(ix)?;

        self.steps.append(handle.pack());

        let ix_prev = self.links.get(link_ix.record_ix(0));
        self.links.append(ix_prev);
        self.links.append(ix.pack());

        self.links.set_pack(link_ix.record_ix(0), new_ix);

        Some(new_ix)
    }
}

pub struct PackedPathRef<'a> {
    path: &'a PackedPath,
    properties: PathPropertyRecord,
}

pub struct PackedPathRefMut<'a> {
    path: &'a mut PackedPath,
    properties: PathPropertyRecord,
}

impl<'a> std::ops::Deref for PackedPathRef<'a> {
    type Target = PackedPath;

    fn deref(&self) -> &PackedPath {
        &self.path
    }
}

impl<'a> std::ops::Deref for PackedPathRefMut<'a> {
    type Target = PackedPath;

    fn deref(&self) -> &PackedPath {
        &self.path
    }
}

impl<'a> std::ops::DerefMut for PackedPathRefMut<'a> {
    fn deref_mut(&mut self) -> &mut PackedPath {
        &mut self.path
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathPropertyRecord {
    head_ptr: PathStepIx,
    tail_ptr: PathStepIx,
    deleted: bool,
    circular: bool,
    deleted_steps: usize,
}

impl PathPropertyRecord {
    fn new(
        head_ptr: PathStepIx,
        tail_ptr: PathStepIx,
        deleted: bool,
        circular: bool,
        deleted_steps: usize,
    ) -> Self {
        Self {
            head_ptr,
            tail_ptr,
            deleted,
            circular,
            deleted_steps,
        }
    }
}

pub struct PathProperties {
    heads: PagedIntVec,
    tails: PagedIntVec,
    deleted: PackedIntVec,
    circular: PackedIntVec,
    deleted_steps: PackedIntVec,
}

impl Default for PathProperties {
    fn default() -> PathProperties {
        Self {
            heads: PagedIntVec::new(super::graph::WIDE_PAGE_WIDTH),
            tails: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
            deleted: Default::default(),
            circular: Default::default(),
            deleted_steps: Default::default(),
        }
    }
}

impl PathProperties {
    fn append_record(&mut self) {
        self.heads.append(0);
        self.tails.append(0);
        self.deleted.append(0);
        self.circular.append(0);
        self.deleted_steps.append(0);
    }

    fn len(&self) -> usize {
        self.heads.len()
    }

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
}

/// A zero-based index into both the corresponding path in the vector
/// of PackedPaths, as well as all the other property records for the
/// path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathNameIx(usize);

impl PathNameIx {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }
}

pub struct PathNames {
    // TODO compress the names; don't store entire Vec<u8>s
    name_id_map: FnvHashMap<Vec<u8>, PathNameIx>,
    names: PackedIntVec,
    lengths: PackedIntVec,
    offsets: PagedIntVec,
}

impl Default for PathNames {
    fn default() -> Self {
        PathNames {
            name_id_map: Default::default(),
            names: Default::default(),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }
}

impl PathNames {
    pub(super) fn add_name(&mut self, name: &[u8]) -> PathNameIx {
        let name_ix = PathNameIx::new(self.lengths.len());

        self.name_id_map.insert(name.into(), name_ix);

        let name_len = name.len() as u64;
        let name_offset = self.lengths.len() as u64;
        self.lengths.append(name_len);
        self.offsets.append(name_offset);

        name.iter().for_each(|&b| self.names.append(b as u64));

        name_ix
    }

    pub(super) fn name_iter(
        &self,
        ix: PathNameIx,
    ) -> Option<packed::vector::Iter<'_>> {
        let vec_ix = ix.0;
        if vec_ix >= self.lengths.len() {
            return None;
        }

        let offset = self.offsets.get_unpack(vec_ix);
        let len = self.lengths.get_unpack(vec_ix);
        let iter = self.names.iter_slice(offset, len);

        Some(iter)
    }
}

pub struct PackedGraphPaths {
    paths: Vec<PackedPath>,
    path_props: PathProperties,
    path_names: PathNames,
}

impl Default for PackedGraphPaths {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            path_props: Default::default(),
            path_names: Default::default(),
        }
    }
}

impl PackedGraphPaths {
    pub(super) fn create_path(&mut self, name: &[u8]) -> PathId {
        let path_id = self.paths.len() as u64;
        let packed_path = PackedPath::new();
        self.paths.push(packed_path);

        self.path_props.append_record();
        let name_ix = self.path_names.add_name(name);

        PathId(path_id)
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub(super) fn path_properties(
        &self,
        id: PathId,
    ) -> Option<PathPropertyRecord> {
        self.path_props.get_record(id)
    }

    pub(super) fn get_path(&self, id: PathId) -> Option<PackedPathRef<'_>> {
        let path = self.paths.get(id.0 as usize)?;
        let properties = self.path_props.get_record(id)?;
        Some(PackedPathRef { path, properties })
    }

    pub(super) fn get_path_mut(
        &mut self,
        id: PathId,
    ) -> Option<PackedPathRefMut<'_>> {
        let path = self.paths.get_mut(id.0 as usize)?;
        let properties = self.path_props.get_record(id)?;
        Some(PackedPathRefMut { path, properties })
    }

    pub(super) fn get_paths_mut<'a, 'b>(
        &'a mut self,
        ids: &'b [PathId],
    ) -> Vec<PackedPathRefMut<'a>> {
        let props = ids
            .iter()
            .copied()
            .filter_map(|i| self.path_props.get_record(i))
            .collect::<Vec<_>>();

        let paths = self
            .paths
            .iter_mut()
            .enumerate()
            .filter_map(|(ix, path)| {
                let ix = PathId(ix as u64);
                if ids.contains(&ix) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        props
            .into_iter()
            .zip(paths.into_iter())
            .map(|(properties, path)| PackedPathRefMut { path, properties })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl PackedList for NodeOccurrences {
    type ListPtr = NodeOccurRecordIx;
    type ListRecord = OccurRecord;

    #[inline]
    fn record_pointer(rec: &OccurRecord) -> NodeOccurRecordIx {
        rec.next
    }

    #[inline]
    fn get_record(&self, ix: NodeOccurRecordIx) -> Option<OccurRecord> {
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

/// The index for a node path occurrence record. Valid indices are
/// natural numbers starting from 1, each denoting a *record*. A zero
/// denotes the end of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeOccurRecordIx(Option<NonZeroUsize>);

crate::impl_one_based_index!(NodeOccurRecordIx);

impl NodeOccurrences {
    pub(super) fn append_record(
        &mut self,
        rec_id: NodeRecordId,
    ) -> Option<NodeOccurRecordIx> {
        let node_rec_ix =
            NodeOccurRecordIx::from_zero_based(self.path_ids.len());

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

    pub(super) fn iter(
        &self,
        ix: NodeOccurRecordIx,
    ) -> PackedListIter<'_, Self> {
        PackedListIter::new(self, ix)
    }

    /*
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
    */
}
