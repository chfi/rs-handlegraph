use crate::{
    handle::{Direction, Edge, Handle, NodeId},
    handlegraph::*,
    mutablehandlegraph::{AdditiveHandleGraph, MutableHandleGraph},
};

use std::num::NonZeroUsize;

use super::graph::{GraphRecordIx, NodeRecordId, RecordIndex};

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

pub struct PathPropertyRecord {
    head_ptr: usize,
    tail_ptr: usize,
    deleted: bool,
    circular: bool,
    deleted_steps: usize,
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
}

// An index into both the offset record and the length record for some
// path name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathNameIx(usize);

impl PathNameIx {
    #[inline]
    pub(super) fn new<I: Into<usize>>(x: I) -> Self {
        Self(x.into())
    }
}

impl RecordIndex for PathNameIx {
    const RECORD_WIDTH: usize = 1;

    fn from_node_record_id(id: NodeRecordId) -> Option<Self> {
        id.to_zero_based().map(PathNameIx)
    }

    fn to_node_record_id(self) -> NodeRecordId {
        NodeRecordId::from_zero_based(self.0)
    }

    fn to_vector_index(self, _: usize) -> usize {
        self.0
    }
}

pub struct PathNames {
    names: PackedIntVec,
    lengths: PackedIntVec,
    offsets: PagedIntVec,
}

impl Default for PathNames {
    fn default() -> Self {
        PathNames {
            names: Default::default(),
            lengths: Default::default(),
            offsets: PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH),
        }
    }
}

impl PathNames {
    pub(super) fn add_name(&mut self, name: &[u8]) -> PathNameIx {
        let name_ix = PathNameIx::new(self.lengths.len());

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
        let vec_ix = ix.to_vector_index(0);
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
        let path_id = self.path_names.lengths.len() as u64;
        let packed_path = PackedPath::new(PathId(path_id));
        self.paths.push(packed_path);

        self.path_props.append_record();
        let name_ix = self.path_names.add_name(name);

        PathId(path_id)
    }
}

/*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeOccurIx(Option<NonZeroUsize>);

impl NodeOccurIx {
    #[inline]
    fn new<I: Into<usize>>(x: I) -> Self {
        Self(NonZeroUsize::new(x.into()))
    }
*/

/*
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
pub(super) fn from_graph_record_ix(g_ix: GraphRecordIx) -> Self {
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
*/
// }

/*
impl RecordIndex for NodeOccurIx {
    const RECORD_WIDTH: usize = 1;

    #[inline]
    fn from_node_record_id(id: NodeRecordId) -> Option<Self> {
        id.to_zero_based().map(NodeOccurIx)
    }

    #[inline]
    fn to_node_record_id(self) -> NodeRecordId {
        NodeRecordId::from_zero_based(self.0)
    }

    #[inline]
    fn to_vector_index(self, _: usize) -> usize {
        self.0
    }
}
*/

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
        rec_id: NodeRecordId,
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
        self.node_occur_offsets.set(ix, offset as u64);
        self.node_occur_next.set(ix, next as u64);

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
