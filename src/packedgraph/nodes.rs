use fnv::{FnvHashMap, FnvHashSet};

use crate::{
    handle::{Direction, Handle, NodeId},
    packed::{self, *},
};

use super::{
    defragment::Defragment,
    edges::EdgeListIx,
    graph::NARROW_PAGE_WIDTH,
    index::{NodeRecordId, OneBasedIndex, RecordIndex},
    occurrences::OccurListIx,
    sequence::{SeqRecordIx, Sequences},
};

/// The index into the underlying packed vector that is used to
/// represent the graph records that hold pointers to the two edge
/// lists for each node.
///
/// Each graph record takes up two elements, so a `GraphVecIx` is
/// always even.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct GraphVecIx(usize);

impl RecordIndex for GraphVecIx {
    const RECORD_WIDTH: usize = 2;

    #[inline]
    fn from_one_based_ix<I: OneBasedIndex>(ix: I) -> Option<Self> {
        ix.to_record_start(Self::RECORD_WIDTH).map(GraphVecIx)
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

impl GraphVecIx {
    #[inline]
    pub(super) fn left_edges_ix(&self) -> usize {
        self.0
    }

    #[inline]
    pub(super) fn right_edges_ix(&self) -> usize {
        self.0 + 1
    }
}

#[derive(Debug, Clone)]
pub struct NodeIdIndexMap {
    pub deque: PackedDeque,
    pub max_id: u64,
    pub min_id: u64,
}

crate::impl_space_usage!(NodeIdIndexMap, [deque]);

impl Default for NodeIdIndexMap {
    fn default() -> Self {
        Self {
            deque: PackedDeque::with_width(4),
            max_id: 0,
            min_id: std::u64::MAX,
        }
    }
}

impl NodeIdIndexMap {
    #[allow(dead_code)]
    pub(crate) fn with_width(width: usize) -> Self {
        let deque = PackedDeque::with_width(width);
        Self {
            deque,
            ..Default::default()
        }
    }

    pub(crate) fn with_width_and_capacity(
        width: usize,
        capacity: usize,
    ) -> Self {
        let deque = PackedDeque::with_width_and_capacity(width, capacity);
        Self {
            deque,
            ..Default::default()
        }
    }

    #[inline]
    fn clear_node_id(&mut self, id: NodeId) {
        let ix = u64::from(id) - self.min_id;
        self.deque.set(ix as usize, 0);
    }

    /// Appends the provided NodeId to the Node id -> Graph index map,
    /// with the given target `GraphRecordIx`.
    ///
    /// Returns `true` if the NodeId was successfully appended.
    #[inline]
    pub fn append_node_id(
        &mut self,
        id: NodeId,
        next_ix: NodeRecordId,
    ) -> bool {
        let id = u64::from(id);
        if id == 0 {
            return false;
        }

        if self.deque.is_empty() {
            self.deque.push_back(0);
        } else {
            if id < self.min_id {
                let to_prepend = self.min_id - id;
                for _ in 0..to_prepend {
                    self.deque.push_front(0);
                }
            }

            if id > self.max_id {
                let ix = (id - self.min_id) as usize;

                if let Some(to_append) = ix.checked_sub(self.deque.len()) {
                    for _ in 0..=to_append {
                        self.deque.push_back(0);
                    }
                }
            }
        }

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = next_ix;

        self.deque.set(index as usize, value.pack());

        true
    }

    #[inline]
    fn has_node<I: Into<NodeId>>(&self, id: I) -> bool {
        self.get_index(id).is_some()
    }

    #[inline]
    pub fn get_index<I: Into<NodeId>>(&self, id: I) -> Option<NodeRecordId> {
        let id = u64::from(id.into());
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let index = id - self.min_id;
        let rec_id: NodeRecordId = self.deque.get_unpack(index as usize);

        if rec_id.is_null() {
            return None;
        }

        Some(rec_id)
    }

    pub(super) fn update_record_indices(
        &mut self,
        record_map: &FnvHashMap<NodeRecordId, NodeRecordId>,
    ) {
        for id in self.min_id..=self.max_id {
            let index = (id - self.min_id) as usize;
            let rec_id: NodeRecordId = self.deque.get_unpack(index);
            if !rec_id.is_null() {
                let new_id = record_map.get(&rec_id).unwrap();
                self.deque.set_pack(index, *new_id);
            }
        }
    }

    pub(super) fn iter(&self) -> IndexMapIter<'_> {
        IndexMapIter {
            iter: self.deque.iter().enumerate(),
            min_id: self.min_id,
        }
    }
}

pub struct IndexMapIter<'a> {
    iter: std::iter::Enumerate<packed::deque::Iter<'a>>,
    min_id: u64,
}

impl<'a> Iterator for IndexMapIter<'a> {
    type Item = NodeId;

    #[inline]
    fn next(&mut self) -> Option<NodeId> {
        let next_non_zero = self.iter.find(|(_, x)| *x != 0)?;
        let id = (next_non_zero.0 as u64) + self.min_id;
        Some(NodeId::from(id))
    }
}

#[derive(Debug, Clone)]
pub struct NodeRecords {
    pub records_vec: PagedIntVec,
    pub id_index_map: NodeIdIndexMap,
    pub sequences: Sequences,
    pub removed_nodes: Vec<NodeRecordId>,
    pub node_occurrence_map: PagedIntVec,
}

crate::impl_space_usage!(
    NodeRecords,
    [
        records_vec,
        id_index_map,
        sequences,
        removed_nodes,
        node_occurrence_map
    ]
);

impl Default for NodeRecords {
    fn default() -> NodeRecords {
        Self {
            records_vec: PagedIntVec::new(NARROW_PAGE_WIDTH),
            id_index_map: Default::default(),
            sequences: Default::default(),
            removed_nodes: Vec::new(),
            node_occurrence_map: PagedIntVec::new(
                super::graph::NARROW_PAGE_WIDTH,
            ),
        }
    }
}

impl Defragment for NodeRecords {
    type Updates = ();

    fn defragment(&mut self) -> Option<Self::Updates> {
        if self.removed_nodes.is_empty() {
            return None;
        }

        let total_len = self.node_count() + self.removed_nodes.len();
        let kept_len = self.node_count();

        let mut records_vec = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let mut node_occurrence_map =
            PagedIntVec::new(super::graph::NARROW_PAGE_WIDTH);

        records_vec.reserve(kept_len * 2);
        node_occurrence_map.reserve(kept_len);

        let mut updates: FnvHashMap<NodeRecordId, NodeRecordId> =
            FnvHashMap::default();

        let removed_nodes = std::mem::take(&mut self.removed_nodes)
            .into_iter()
            .collect::<FnvHashSet<_>>();

        let mut next_ix = 0usize;

        for ix in 0..total_len {
            let rec_id = NodeRecordId::from_zero_based(ix);
            if !removed_nodes.contains(&rec_id) {
                let new_rec_id = NodeRecordId::from_zero_based(next_ix);

                updates.insert(rec_id, new_rec_id);

                let rec_vec_ix = rec_id.to_record_start(2).unwrap();
                let occur_vec_ix = rec_id.to_record_start(1).unwrap();

                let left_ix: EdgeListIx =
                    self.records_vec.get_unpack(rec_vec_ix);
                let right_ix: EdgeListIx =
                    self.records_vec.get_unpack(rec_vec_ix + 1);

                let occur_ix: OccurListIx =
                    self.node_occurrence_map.get_unpack(occur_vec_ix);

                records_vec.append(left_ix.pack());
                records_vec.append(right_ix.pack());
                node_occurrence_map.append(occur_ix.pack());

                next_ix += 1;
            }
        }
        self.id_index_map.update_record_indices(&updates);

        self.records_vec = records_vec;
        self.node_occurrence_map = node_occurrence_map;
        self.sequences.defragment();

        Some(())
    }
}

impl NodeRecords {
    pub(crate) fn with_expected_node_count(nodes: usize) -> Self {
        let width = 64 - nodes.leading_zeros() as usize;
        let id_index_map =
            NodeIdIndexMap::with_width_and_capacity(width, nodes);

        Self {
            id_index_map,
            ..Default::default()
        }
    }

    pub(super) fn transform_node_ids<F>(&mut self, mut new_id_fn: F)
    where
        F: FnMut(NodeId) -> NodeId,
    {
        let mut new_index_map = NodeIdIndexMap::default();
        new_index_map.deque.reserve(self.node_count());

        let min_id = self.min_id();
        for i in 0..self.id_index_map.deque.len() {
            let i = i as u64;
            let old_id = NodeId::from((i as u64) + min_id);
            if let Some(rec_id) = self.id_index_map.get_index(old_id) {
                let new_id = new_id_fn(old_id);
                new_index_map.append_node_id(new_id, rec_id);
            }
        }

        self.id_index_map = new_index_map;
    }

    #[inline]
    pub fn min_id(&self) -> u64 {
        self.id_index_map.min_id
    }

    #[inline]
    pub fn max_id(&self) -> u64 {
        self.id_index_map.max_id
    }

    pub(super) fn node_ids_iter(&self) -> IndexMapIter<'_> {
        self.id_index_map.iter()
    }

    #[inline]
    pub fn has_node<I: Into<NodeId>>(&self, id: I) -> bool {
        self.id_index_map.has_node(id)
    }

    #[inline]
    pub fn node_count(&self) -> usize {
        (self.records_vec.len() / 2) - self.removed_nodes.len()
    }

    #[inline]
    pub fn total_length(&self) -> usize {
        self.sequences.total_length()
    }

    /// Return the `GraphRecordIx` that will be used by the next node
    /// that's inserted into the graph.
    #[inline]
    fn next_graph_ix(&self) -> NodeRecordId {
        let rec_count = self.records_vec.len();
        NodeRecordId::from_record_start(rec_count, 2)
    }

    #[inline]
    pub(super) fn sequences(&self) -> &Sequences {
        &self.sequences
    }

    #[inline]
    pub(super) fn sequences_mut(&mut self) -> &mut Sequences {
        &mut self.sequences
    }

    /// Append a new node graph record, using the provided
    /// `NodeRecordId` no ensure that the record index is correctly
    /// synced.
    #[must_use]
    fn append_node_graph_record(
        &mut self,
        g_rec_ix: NodeRecordId,
    ) -> Option<NodeRecordId> {
        if self.next_graph_ix() != g_rec_ix {
            return None;
        }
        self.records_vec.append(0);
        self.records_vec.append(0);
        self.node_occurrence_map.append(0);
        Some(g_rec_ix)
    }

    #[inline]
    fn insert_node(&mut self, n_id: NodeId) -> Option<NodeRecordId> {
        if n_id == NodeId::from(0) {
            return None;
        }

        let next_ix = self.next_graph_ix();

        // Make sure the node ID is valid and doesn't already exist
        if !self.id_index_map.append_node_id(n_id, next_ix) {
            return None;
        }

        // append the sequence and graph records
        self.sequences.append_empty_record();
        let record_ix = self.append_node_graph_record(next_ix)?;

        Some(record_ix)
    }

    #[inline]
    pub fn get_node_seq_range(&self, handle: Handle) -> Option<(usize, usize)> {
        let rec_id = self.handle_record(handle)?;
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id)?;
        Some(self.sequences.get_record(seq_ix))
    }

    pub(super) fn clear_node_record(&mut self, n_id: NodeId) -> Option<()> {
        let rec_id = self.id_index_map.get_index(n_id)?;

        let occ_map_ix = rec_id.to_record_ix(1, 0)?;
        let rec_ix = rec_id.to_record_ix(2, 0)?;
        let seq_ix = SeqRecordIx::from_one_based_ix(rec_id)?;

        // clear node occurrence heads
        self.node_occurrence_map.set(occ_map_ix, 0);

        // clear node record/edge list heads
        self.records_vec.set(rec_ix, 0);
        self.records_vec.set(rec_ix, 1);

        // clear sequence record
        self.sequences.clear_record(seq_ix);

        self.id_index_map.clear_node_id(n_id);

        self.removed_nodes.push(rec_id);

        Some(())
    }

    #[inline]
    pub(super) fn get_edge_list(
        &self,
        rec_id: NodeRecordId,
        dir: Direction,
    ) -> EdgeListIx {
        match GraphVecIx::from_one_based_ix(rec_id) {
            None => EdgeListIx::null(),
            Some(vec_ix) => {
                let ix = match dir {
                    Direction::Right => vec_ix.right_edges_ix(),
                    Direction::Left => vec_ix.left_edges_ix(),
                };

                self.records_vec.get_unpack(ix)
            }
        }
    }

    #[inline]
    pub(super) fn set_edge_list(
        &mut self,
        rec_id: NodeRecordId,
        dir: Direction,
        new_edge: EdgeListIx,
    ) -> Option<()> {
        let vec_ix = GraphVecIx::from_one_based_ix(rec_id)?;

        let ix = match dir {
            Direction::Right => vec_ix.right_edges_ix(),
            Direction::Left => vec_ix.left_edges_ix(),
        };

        self.records_vec.set_pack(ix, new_edge);
        Some(())
    }

    #[inline]
    pub(super) fn get_node_edge_lists(
        &self,
        rec_id: NodeRecordId,
    ) -> Option<(EdgeListIx, EdgeListIx)> {
        let vec_ix = GraphVecIx::from_one_based_ix(rec_id)?;

        let left = vec_ix.left_edges_ix();
        let left = self.records_vec.get_unpack(left);

        let right = vec_ix.right_edges_ix();
        let right = self.records_vec.get_unpack(right);

        Some((left, right))
    }

    #[allow(dead_code)]
    pub(super) fn set_node_edge_lists(
        &mut self,
        rec_id: NodeRecordId,
        left: EdgeListIx,
        right: EdgeListIx,
    ) -> Option<()> {
        let vec_ix = GraphVecIx::from_one_based_ix(rec_id)?;

        let left_ix = vec_ix.left_edges_ix();
        let right_ix = vec_ix.right_edges_ix();
        self.records_vec.set_pack(left_ix, left);
        self.records_vec.set_pack(right_ix, right);

        Some(())
    }

    #[inline]
    pub(super) fn update_node_edge_lists<F>(
        &mut self,
        rec_id: NodeRecordId,
        f: F,
    ) -> Option<()>
    where
        F: Fn(EdgeListIx, EdgeListIx) -> (EdgeListIx, EdgeListIx),
    {
        let vec_ix = GraphVecIx::from_one_based_ix(rec_id)?;

        let (left_rec, right_rec) = self.get_node_edge_lists(rec_id)?;

        let (new_left, new_right) = f(left_rec, right_rec);

        let left_ix = vec_ix.left_edges_ix();
        let right_ix = vec_ix.right_edges_ix();
        self.records_vec.set_pack(left_ix, new_left);
        self.records_vec.set_pack(right_ix, new_right);

        Some(())
    }

    #[inline]
    pub(super) fn create_node<I: Into<NodeId>>(
        &mut self,
        n_id: I,
        seq: &[u8],
    ) -> Option<NodeRecordId> {
        let n_id = n_id.into();
        // update the node ID/graph index map
        let g_ix = self.insert_node(n_id)?;

        // insert the sequence
        self.sequences.add_sequence(g_ix, seq);

        Some(g_ix)
    }

    #[inline]
    pub(super) fn append_empty_node(&mut self) -> NodeId {
        let n_id = NodeId::from(self.id_index_map.max_id + 1);
        let _g_ix = self.insert_node(n_id).unwrap();
        n_id
    }

    #[inline]
    pub(crate) fn handle_record(&self, h: Handle) -> Option<NodeRecordId> {
        self.id_index_map.get_index(h.id())
    }

    #[inline]
    pub(crate) fn node_record_occur(
        &self,
        rec_id: NodeRecordId,
    ) -> Option<OccurListIx> {
        let vec_ix = rec_id.to_zero_based()?;
        Some(self.node_occurrence_map.get_unpack(vec_ix))
    }

    /// Maps a handle into its corresponding occurrence record
    /// pointer, if the node for the handle exists in the PackedGraph.
    #[inline]
    pub(crate) fn handle_occur_record(&self, h: Handle) -> Option<OccurListIx> {
        self.handle_record(h)
            .and_then(|r| self.node_record_occur(r))
    }

    pub(crate) fn apply_edge_lists_ix_updates(
        &mut self,
        updates: &FnvHashMap<EdgeListIx, EdgeListIx>,
    ) {
        let total_len = self.node_count() + self.removed_nodes.len();

        for ix in 0..total_len {
            let vec_ix = ix * 2;

            let old_left: EdgeListIx = self.records_vec.get_unpack(vec_ix);
            let old_right: EdgeListIx = self.records_vec.get_unpack(vec_ix + 1);

            if !old_left.is_null() {
                let left = updates.get(&old_left).unwrap();
                self.records_vec.set_pack(vec_ix, *left);
            }

            if !old_right.is_null() {
                let right = updates.get(&old_right).unwrap();
                self.records_vec.set_pack(vec_ix + 1, *right);
            }
        }
    }

    pub(crate) fn apply_node_occur_ix_updates(
        &mut self,
        updates: &FnvHashMap<OccurListIx, OccurListIx>,
    ) {
        let total_len = self.node_count() + self.removed_nodes.len();

        for ix in 0..total_len {
            let old_head: OccurListIx = self.node_occurrence_map.get_unpack(ix);

            if !old_head.is_null() {
                let head = updates.get(&old_head).unwrap();
                self.node_occurrence_map.set_pack(ix, *head);
            }
        }
    }

    pub(crate) fn apply_edge_and_occur_updates(
        &mut self,
        edge_updates: &FnvHashMap<EdgeListIx, EdgeListIx>,
        occur_updates: &FnvHashMap<OccurListIx, OccurListIx>,
    ) {
        let total_len = self.node_count() + self.removed_nodes.len();

        for ix in 0..total_len {
            let vec_ix = ix * 2;

            let old_left: EdgeListIx = self.records_vec.get_unpack(vec_ix);
            let old_right: EdgeListIx = self.records_vec.get_unpack(vec_ix + 1);

            if !old_left.is_null() {
                let left = edge_updates.get(&old_left).unwrap();
                self.records_vec.set_pack(vec_ix, *left);
            }

            if !old_right.is_null() {
                let right = edge_updates.get(&old_right).unwrap();
                self.records_vec.set_pack(vec_ix + 1, *right);
            }

            let old_head: OccurListIx = self.node_occurrence_map.get_unpack(ix);

            if !old_head.is_null() {
                let head = occur_updates.get(&old_head).unwrap();
                self.node_occurrence_map.set_pack(ix, *head);
            }
        }
    }

    pub fn save_diagnostics(&self, path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;

        writeln!(&mut file, "# min_id {:6}", self.id_index_map.min_id)?;
        writeln!(&mut file, "# max_id {:6}", self.id_index_map.max_id)?;
        self.id_index_map.deque.save_diagnostics(&mut file)?;
        self.records_vec.save_diagnostics(&mut file)?;

        Ok(())
    }

    pub fn print_diagnostics(&self) {
        println!("\n ~~ BEGIN NodeRecords diagnostics ~~ \n");

        println!(" ----- {:^20} -----", "id_index_map");
        println!(
            "  min_id: {:8}\t max_id: {:8}",
            self.id_index_map.min_id, self.id_index_map.max_id
        );
        self.id_index_map.deque.print_diagnostics();
        println!();

        println!(" ----- {:^20} -----", "records_vec");
        self.records_vec.print_diagnostics();
        println!();

        println!(" ----- {:^20} -----", "sequences");
        // print!("  sequences: ");
        // self.sequences.sequences.print_diagnostics();
        print!("  lengths:   ");
        self.sequences.lengths.print_diagnostics();
        println!("  offsets:");
        self.sequences.offsets.print_diagnostics();
        println!();

        println!(" ----- {:^20} -----", "node_occurrence_map");
        self.node_occurrence_map.print_diagnostics();
        println!();

        println!("\n ~~  END  NodeRecords diagnostics ~~ \n");
    }
}
