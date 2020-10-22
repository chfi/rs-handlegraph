use gfa::{
    gfa::{Link, Orientation, Segment, GFA},
    optfields::OptFields,
};

use crate::{
    handle::{Edge, Handle, NodeId},
    packed::*,
};

static NARROW_PAGE_WIDTH: usize = 256;
static WIDE_PAGE_WIDTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct Sequences {
    sequences: PackedIntVec,
    lengths: PackedIntVec,
    indices: PagedIntVec,
}

const fn encode_dna_base(base: u8) -> u64 {
    match base {
        b'a' | b'A' => 0,
        b'c' | b'C' => 1,
        b'g' | b'G' => 2,
        b't' | b'T' => 3,
        _ => 4,
    }
}

const fn encoded_complement(val: u64) -> u64 {
    if val == 4 {
        4
    } else {
        3 - val
    }
}

const fn decode_dna_base(byte: u64) -> u8 {
    match byte {
        0 => b'A',
        1 => b'C',
        2 => b'G',
        3 => b'T',
        _ => b'N',
    }
}

impl Sequences {
    const SIZE: usize = 1;

    pub(super) fn add_record(&mut self, ix: usize, seq: &[u8]) {
        let seq_ix = self.sequences.len();
        self.indices.set(ix, seq_ix as u64);
        self.lengths.set(ix, seq.len() as u64);
        seq.iter()
            .for_each(|&b| self.sequences.append(encode_dna_base(b)));
    }

    pub(super) fn get_sequence(&self, ix: usize) -> Vec<u8> {
        let start = self.indices.get(ix) as usize;
        let len = self.lengths.get(ix) as usize;
        let mut seq = Vec::with_capacity(len);
        for i in 0..len {
            let base = self.sequences.get(start + i);
            seq.push(decode_dna_base(base));
        }
        seq
    }

    pub(super) fn length(&self, ix: usize) -> usize {
        self.lengths.get(ix) as usize
    }
}

impl Default for Sequences {
    fn default() -> Self {
        Sequences {
            sequences: Default::default(),
            lengths: Default::default(),
            indices: PagedIntVec::new(NARROW_PAGE_WIDTH),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EdgeRecord {
    handle: Handle,
    next: EdgeIx,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct EdgeIx(usize);

impl EdgeIx {
    #[inline]
    pub(super) fn from_edge_list_ix(ix: usize) -> Self {
        EdgeIx(ix / EdgeLists::RECORD_SIZE)
    }

    #[inline]
    pub(super) fn to_edge_list_ix(&self) -> usize {
        (self.0 - 1) * EdgeLists::RECORD_SIZE
    }
}

#[derive(Debug, Clone)]
pub struct EdgeLists {
    pub(super) edge_lists: PagedIntVec,
}

impl Default for EdgeLists {
    fn default() -> Self {
        EdgeLists {
            edge_lists: PagedIntVec::new(WIDE_PAGE_WIDTH),
        }
    }
}

impl EdgeLists {
    pub(super) const RECORD_SIZE: usize = 2;
    pub(super) fn append_record(
        &mut self,
        handle: Handle,
        next: EdgeIx,
    ) -> EdgeIx {
        let ix = EdgeIx::from_edge_list_ix(self.edge_lists.len());
        self.edge_lists.append(handle.as_integer());
        let next = next.to_edge_list_ix();
        self.edge_lists.append(next as u64);
        ix
    }

    pub(super) fn get_record(&self, ix: EdgeIx) -> EdgeRecord {
        let ix = ix.to_edge_list_ix();
        let handle = Handle::from_integer(self.edge_lists.get(ix));
        let next = EdgeIx(self.edge_lists.get(ix + 1) as usize);
        EdgeRecord { handle, next }
    }

    pub(super) fn next(&self, rec: EdgeRecord) -> Option<EdgeRecord> {
        if rec.next.0 != 0 {
            Some(self.get_record(rec.next))
        } else {
            None
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct GraphRecord {
    edges_start: usize,
    edges_end: usize,
}

impl GraphRecord {
    pub(super) const SIZE: usize = 2;
    pub(super) const START_OFFSET: usize = 0;
    pub(super) const END_OFFSET: usize = 1;
}

#[derive(Debug, Clone)]
pub struct PackedGraph {
    pub(super) graph_records: PagedIntVec,
    pub(super) sequences: Sequences,
    pub(super) edges: EdgeLists,
    pub(super) id_graph_map: PackedDeque,
    pub(super) max_id: u64,
    pub(super) min_id: u64,
}

impl Default for PackedGraph {
    fn default() -> Self {
        let sequences = Default::default();
        let edges = Default::default();
        let id_graph_map = Default::default();
        let graph_records = PagedIntVec::new(NARROW_PAGE_WIDTH);
        let max_id = 0;
        let min_id = std::u64::MAX;
        PackedGraph {
            sequences,
            edges,
            graph_records,
            id_graph_map,
            max_id,
            min_id,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct GraphIx(usize);

impl GraphIx {
    pub(super) fn to_id_map_entry(&self) -> u64 {
        let ix = self.0 as u64;
        ix + 1
    }

    pub(super) fn from_id_map_entry(ix: u64) -> Option<Self> {
        if ix == 0 {
            None
        } else {
            Some(GraphIx((ix - 1) as usize))
        }
    }

    pub(super) fn from_graph_records_ix(ix: usize) -> Self {
        GraphIx(ix / GraphRecord::SIZE)
    }

    pub(super) fn to_seq_record_ix(&self) -> usize {
        let ix = self.0;
        (ix * Sequences::SIZE) / GraphRecord::SIZE
    }

    pub(super) fn start_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::START_OFFSET
    }

    pub(super) fn end_edges_ix(&self) -> usize {
        let ix = self.0;
        (ix * GraphRecord::SIZE) + GraphRecord::END_OFFSET
    }
}

impl PackedGraph {
    pub fn new() -> Self {
        Default::default()
    }

    pub(super) fn new_record_ix(&mut self) -> GraphIx {
        let new_ix = self.graph_records.len();
        self.graph_records.append(0);
        self.graph_records.append(0);
        self.sequences.lengths.append(0);
        self.sequences.indices.append(0);
        GraphIx::from_graph_records_ix(new_ix)
    }

    pub(super) fn get_graph_record(&self, ix: GraphIx) -> GraphRecord {
        let edges_start = self.graph_records.get(ix.start_edges_ix()) as usize;
        let edges_end = self.graph_records.get(ix.end_edges_ix()) as usize;
        GraphRecord {
            edges_start,
            edges_end,
        }
    }

    pub(super) fn get_node_index(&self, id: NodeId) -> Option<GraphIx> {
        let id = u64::from(id);
        if id < self.min_id || id > self.max_id {
            return None;
        }
        let map_ix = id - self.min_id;
        let ix = self.id_graph_map.get(map_ix as usize);
        GraphIx::from_id_map_entry(ix)
    }

    pub(super) fn handle_graph_ix(&self, handle: Handle) -> Option<GraphIx> {
        self.get_node_index(handle.id())
    }

    pub(super) fn push_node_record(&mut self, id: NodeId) -> GraphIx {
        let next_ix = self.new_record_ix();

        if self.id_graph_map.is_empty() {
            self.id_graph_map.push_back(0);
        } else {
            let id = u64::from(id);
            if id < self.min_id {
                let to_prepend = self.min_id - id;
                for _ in 0..to_prepend {
                    self.id_graph_map.push_front(0);
                }
            }
            if id > self.max_id {
                let to_append =
                    self.id_graph_map.len() - (id - self.min_id) as usize;
                for _ in 0..to_append {
                    self.id_graph_map.push_back(0);
                }
            }
        }

        let id = u64::from(id);

        self.min_id = self.min_id.min(id);
        self.max_id = self.max_id.max(id);

        let index = id - self.min_id;
        let value = next_ix.to_id_map_entry();

        self.id_graph_map.set(index as usize, value);

        next_ix
    }

    pub fn create_handle(&mut self, sequence: &[u8], id: NodeId) -> Handle {
        assert!(!sequence.is_empty() && id != NodeId::from(0));

        // todo make sure the node doesn't already exist

        let graph_ix = self.push_node_record(id);
        let seq_ix = graph_ix.to_seq_record_ix();

        self.sequences.add_record(seq_ix, sequence);

        Handle::pack(id, false)
    }

    pub fn append_handle(&mut self, sequence: &[u8]) -> Handle {
        let id = NodeId::from(self.max_id + 1);
        self.create_handle(sequence, id)
    }

    #[inline]
    fn get_edge_list_entry(&self, ix: usize) -> EdgeIx {
        let entry = self.graph_records.get(ix);
        EdgeIx::from_edge_list_ix(entry as usize)
    }

    pub fn create_edge(&mut self, left: Handle, right: Handle) -> Option<()> {
        let left_g_ix = self.handle_graph_ix(left)?;
        let right_g_ix = self.handle_graph_ix(right)?;

        let left_edge_g_ix = if left.is_reverse() {
            left_g_ix.start_edges_ix()
        } else {
            left_g_ix.end_edges_ix()
        };

        let right_edge_g_ix = if right.is_reverse() {
            right_g_ix.end_edges_ix()
        } else {
            right_g_ix.start_edges_ix()
        };

        let right_next = self.get_edge_list_entry(left_edge_g_ix);
        let edge_ix = self.edges.append_record(right, right_next);

        // self.graph_records.set(left_edge_g_ix, edge_ix.0 as u64);
        self.graph_records
            .set(left_edge_g_ix, edge_ix.to_edge_list_ix() as u64);

        if left_edge_g_ix == right_edge_g_ix {
            // todo reversing self edge records?
            return Some(());
        }

        let left_next = self.get_edge_list_entry(right_edge_g_ix);
        let edge_ix = self.edges.append_record(left.flip(), left_next);

        // self.graph_records.set(right_edge_g_ix, edge_ix.0 as u64);
        self.graph_records
            .set(right_edge_g_ix, edge_ix.to_edge_list_ix() as u64);

        Some(())
    }
}
