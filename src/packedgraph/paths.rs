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

        self.links.append(ix.as_vec_value());
        self.links.append(0);

        if !self.tail.is_null() {
            // this is definitely super wrong
            self.links.set(
                (ix.as_vec_value() - 1) as usize,
                self.tail.to_zero_based().unwrap() as u64,
            );
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
        self.links.set(
            (ix.as_vec_value() - 1) as usize,
            self.tail.to_zero_based().unwrap() as u64,
        );
        // }

        ix
    }
}

// impl<'a> PathRef for &'a PackedPath {

// }

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

    fn next(&mut self) -> Option<(usize, Handle)> {
        if self.finished {
            return None;
        }

        let handle = Handle::from_integer(self.steps.get(self.current_step));
        let index = self.current_step;

        let link = self.current_step += 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathPropertyRecord {
    head_ptr: usize,
    tail_ptr: usize,
    deleted: bool,
    circular: bool,
    deleted_steps: usize,
}

impl PathPropertyRecord {
    fn new(
        head_ptr: usize,
        tail_ptr: usize,
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
        self.heads.set(ix, record.head_ptr as u64);
        self.tails.set(ix, record.tail_ptr as u64);
        self.deleted.set(ix, record.deleted as u64);
        self.circular.set(ix, record.circular as u64);
        self.deleted_steps.set(ix, record.deleted_steps as u64);
        true
    }

    fn get_record(&self, id: PathId) -> Option<PathPropertyRecord> {
        if id.0 >= self.len() as u64 {
            return None;
        }
        let ix = id.0 as usize;
        let head_ptr = self.heads.get(ix) as usize;
        let tail_ptr = self.tails.get(ix) as usize;
        let deleted = self.deleted.get(ix) == 1;
        let circular = self.circular.get(ix) == 1;
        let deleted_steps = self.deleted_steps.get(ix) as usize;

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
    ) -> Option<PackedIntVecIter<'_>> {
        let vec_ix = ix.0;
        if vec_ix >= self.lengths.len() {
            return None;
        }

        let offset = self.offsets.get(vec_ix) as usize;
        let len = self.lengths.get(vec_ix) as usize;
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
        let packed_path = PackedPath::new(PathId(path_id));
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

    pub(super) fn get_path(&self, id: PathId) -> Option<&PackedPath> {
        self.paths.get(id.0 as usize)
    }

    pub(super) fn get_path_mut(
        &mut self,
        id: PathId,
    ) -> Option<&mut PackedPath> {
        self.paths.get_mut(id.0 as usize)
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
        let offset = self.node_occur_offsets.get(ix) as usize;
        let next =
            NodeOccurRecordIx::from_vector_value(self.node_occur_next.get(ix));

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

            self.path_ids.set(ix, path_id.0);
            self.node_occur_offsets.set(ix, offset as u64);
            self.node_occur_next.set(ix, next.to_vector_value());

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
